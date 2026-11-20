"""Independent TCP interoperability checks for a local Coral binary (Python standard library)."""
import contextlib
import hashlib
import hmac
import json
import os
from pathlib import Path
import socket
import sqlite3
import struct
import subprocess
import tempfile
import time
import unittest
import urllib.request
import uuid
import zlib

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("CORAL_BIN", ROOT / "target" / "debug" / ("coral.exe" if os.name == "nt" else "coral"))).resolve()


def var(n):
    n &= 0xFFFFFFFF
    b = bytearray()
    while n >= 128:
        b.append((n & 127) | 128)
        n >>= 7
    b.append(n)
    return bytes(b)


def number(data, offset=0):
    value = 0
    for i in range(5):
        b = data[offset + i]
        value |= (b & 127) << (7 * i)
        if b < 128:
            return value, offset + i + 1
    raise ValueError("invalid VarInt")


def string(text):
    raw = text.encode()
    return var(len(raw)) + raw


def text_at(data, offset=0):
    size, offset = number(data, offset)
    return data[offset:offset + size].decode(), offset + size


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Client:
    def __init__(self, port, state, host="localhost"):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=4)
        self.threshold = None
        self.send(0, var(47) + string(host) + struct.pack(">H", port) + var(state))

    def send(self, pid, data=b""):
        body = var(pid) + data
        if self.threshold is not None:
            body = var(len(body)) + zlib.compress(body) if len(body) >= self.threshold else b"\0" + body
        self.s.sendall(var(len(body)) + body)

    def read(self):
        prefix = bytearray()
        while True:
            b = self.s.recv(1)
            if not b:
                raise EOFError()
            prefix += b
            if b[0] < 128:
                break
        size, _ = number(prefix)
        data = bytearray()
        while len(data) < size:
            part = self.s.recv(size - len(data))
            if not part:
                raise EOFError()
            data += part
        if self.threshold is not None:
            declared, offset = number(data)
            data = data[offset:]
            if declared:
                assert declared >= self.threshold
                data = zlib.decompress(data)
                assert len(data) == declared
            else:
                assert len(data) < self.threshold
        pid, start = number(data)
        return pid, bytes(data[start:])

    def until(self, pid, predicate=lambda _: True):
        end = time.monotonic() + 5
        while time.monotonic() < end:
            kind, body = self.read()
            if kind == pid and predicate(body):
                return body
            if self.threshold is not None and kind == 0:
                self.send(0, body)
        raise AssertionError(f"packet {pid:#x} missing")

    def login(self, name):
        self.send(0, string(name))
        self.threshold, _ = number(self.until(3))
        self.until(2)
        self.until(1)

    def confirm_pos(self):
        pos = self.until(8)
        self.send(6, pos[:32] + b"\1")

    def chat(self, command):
        self.send(1, string(command))


@contextlib.contextmanager
def server(prepare_db=None, **overrides):
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        port, metrics = free_port(), free_port()
        while metrics == port:
            metrics = free_port()
        arena = tmp / "arena.car"
        subprocess.run([BINARY, "arena-create", arena], check=True)
        name = "Alice"
        uid = uuid.UUID(bytes=hashlib.md5(f"OfflinePlayer:{name}".encode()).digest(), version=3)
        config = dict(listen=f"127.0.0.1:{port}", metrics=f"127.0.0.1:{metrics}", data_dir=str(tmp / "data"),
                      arena_file=str(arena), operators={str(uid): 4}, view_distance=1)
        config.update(overrides)
        path = tmp / "config.json"
        env = dict(os.environ, CORAL_TEST_SECRET="regression-test-secret-of-at-least-32-bytes")
        if os.name == "nt":
            config["database_url_env"] = "CORAL_TEST_DB"
            env["CORAL_TEST_DB"] = "sqlite://profiles.sqlite?mode=rwc"
        path.write_text(json.dumps(config))
        with subprocess.Popen([BINARY, "--config", path], cwd=tmp, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as proc:
            try:
                for _ in range(100):
                    if proc.poll() is not None:
                        raise RuntimeError(proc.stderr.read().decode())
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=.1):
                            break
                    except OSError:
                        time.sleep(.04)
                if prepare_db:
                    db_path = tmp / ("profiles.sqlite" if os.name == "nt" else "data/profiles.sqlite")
                    with contextlib.closing(sqlite3.connect(db_path)) as db:
                        prepare_db(db)
                        db.commit()
                yield port, metrics
            finally:
                proc.terminate()
                proc.wait(timeout=5)


class PlayTests(unittest.TestCase):
    def test_profile_command_grants(self):
        def grant(db):
            uid = uuid.UUID(bytes=hashlib.md5(b"OfflinePlayer:Bob").digest(), version=3)
            db.execute("INSERT INTO coral_permissions(uuid,level,nodes) VALUES(?,?,?)",
                       (str(uid), 1, '["coral.command.give"]'))

        with server(prepare_db=grant, operators={}) as (port, _):
            b = self.client(port)
            b.login("Bob")
            b.confirm_pos()
            b.chat("/say hello")
            self.assertIn("hello", text_at(b.until(2))[0])
            b.send(0x14, string("/giv") + b"\0")
            self.assertEqual(b.until(0x3a), var(1) + string("/give"))
            b.chat("/give Bob 1")
            self.assertEqual(b.until(0x2f), bytes.fromhex("00 0009 0001 01 0000 00"))
            b.chat("/seed")
            self.assertIn("Permission denied", text_at(b.until(2))[0])

    def client(self, port, state=2, host="localhost"):
        c = Client(port, state, host)
        self.addCleanup(c.s.close)
        return c

    def test_two_players_and_world_transfer(self):
        with server() as (port, metrics):
            status = self.client(port, 1)
            status.send(0)
            self.assertEqual(json.loads(text_at(status.until(0))[0])["version"]["protocol"], 47)
            token = struct.pack(">q", 17819314)
            status.send(1, token)
            self.assertEqual(status.until(1), token)
            a = self.client(port)
            a.login("Alice")
            a.confirm_pos()
            chunk = a.until(0x21)
            cx, cz, full, mask = struct.unpack(">iiBH", chunk[:11])
            self.assertEqual((cx, cz, full, mask), (0, 0, 1, 1))
            length, start = number(chunk, 11)
            self.assertEqual(length, 12288 + 256)
            self.assertEqual(len(chunk[start:]), length)
            b = self.client(port)
            b.login("Bob")
            b.confirm_pos()
            a.until(0x0c)
            a.chat('hello Bob')
            self.assertIn("hello Bob", text_at(b.until(2))[0])
            b.send(0x14, string("/world tp ar") + b"\0")
            self.assertEqual(b.until(0x3a), var(1) + string("arena"))
            a.chat("/give Bob 1 3")
            slot = b.until(0x2f)
            self.assertEqual(struct.unpack(">bhhBhB", slot), (0, 9, 1, 3, 0, 0))
            b.send(0x16, var(1))
            self.assertIn(b"stat.playOneMinute", b.until(0x37))
            b.chat("/world tp arena")
            self.assertEqual(struct.unpack(">i", b.until(7)[:4])[0], -1)
            self.assertEqual(struct.unpack(">i", b.until(7)[:4])[0], 0)
            b.confirm_pos()
            a.until(0x13)
            with urllib.request.urlopen(f"http://127.0.0.1:{metrics}/metrics") as r:
                self.assertIn("coral_players 2", r.read().decode())

    def test_number_key_and_double_click(self):
        with server() as (port, _):
            a = self.client(port)
            a.login("Alice")
            a.confirm_pos()
            a.chat("/give Alice 1 3")
            self.assertEqual(a.until(0x2f), bytes.fromhex("00 0009 0001 03 0000 00"))
            for click, ack, slots in [
                ("00 0009 08 0001 02 ffff", "00 0001 01",
                 ["00 0009 ffff", "00 002c 0001 03 0000 00", "ff ffff ffff"]),
                ("00 002c 01 0002 00 0001 03 0000 00", "00 0002 01",
                 ["00 002c 0001 01 0000 00", "ff ffff 0001 02 0000 00"]),
                ("00 0009 00 0003 06 ffff", "00 0003 01",
                 ["00 002c ffff", "ff ffff 0001 03 0000 00"]),
            ]:
                a.send(0x0e, bytes.fromhex(click))
                self.assertEqual(a.until(0x32), bytes.fromhex(ack))
                for slot in slots:
                    self.assertEqual(a.until(0x2f), bytes.fromhex(slot))

    def test_queue_promotion(self):
        with server(max_players=1) as (port, _):
            a = self.client(port)
            a.login("Alice")
            a.confirm_pos()
            b = self.client(port)
            b.login("Bob")
            self.assertIn("Waiting", text_at(b.until(2))[0])
            a.s.close()
            b.until(7)
            b.confirm_pos()
            b.until(0x21)

    def test_signed_forwarding_and_replay(self):
        with server(proxy_secret_env="CORAL_TEST_SECRET") as (port, _):
            raw = "\0".join(["Coral47/1", "Alice", str(uuid.uuid4()), "192.0.2.1", str(int(time.time())), str(uuid.uuid4())])
            signature = hmac.new(b"regression-test-secret-of-at-least-32-bytes", raw.encode(), hashlib.sha256).hexdigest()
            host = raw + "\0" + signature
            a = self.client(port, host=host)
            a.login("Alice")
            a.s.close()
            b = self.client(port, host=host)
            b.send(0, string("Alice"))
            with self.assertRaises((EOFError, ConnectionError)):
                b.read()

if __name__ == "__main__":
    unittest.main()
