"""Time the first complete lit Overworld view over loopback Protocol 47."""
import argparse
import hashlib
import struct
import tempfile
import time
from pathlib import Path

from protocol47 import Client, number, server


def measure(radius):
    with tempfile.TemporaryDirectory() as world:
        Path(world, "seed").write_text("4862413858852300341")
        with server(arena_file=None, terrain="normal", worlds=["overworld"],
                    view_distance=radius, data_dir=world) as (port, _):
            client = Client(port, 2)
            try:
                client.login("LightBench")
                client.confirm_pos()
                start = time.perf_counter()
                columns = {}
                first_ms = None
                while len(columns) < (2 * radius + 1) ** 2:
                    pid, data = client.read()
                    if pid == 0:
                        client.send(0, data)
                        continue
                    if pid == 0x21:
                        x, z, full, mask = struct.unpack_from(">iiBH", data)
                        assert full and mask
                        size, offset = number(data, 11)
                        assert size == len(data) - offset
                        entries = [(x, z, mask)]
                    elif pid == 0x26:
                        assert data[0] == 1
                        count, offset = number(data, 1)
                        entries = [struct.unpack_from(">iiH", data, offset + n * 10)
                                   for n in range(count)]
                        offset += count * 10
                    else:
                        continue
                    for x, z, mask in entries:
                        n = mask.bit_count()
                        end = offset + n * 12288 + 256
                        assert end <= len(data) and (x, z) not in columns
                        columns[x, z] = hashlib.sha256(data[offset + n * 8192:end - 256]).digest()
                        offset = end
                    assert offset == len(data)
                    if first_ms is None:
                        first_ms = (time.perf_counter() - start) * 1000
                total_ms = (time.perf_counter() - start) * 1000
                signature = hashlib.sha256()
                for pos, light in sorted(columns.items()):
                    signature.update(struct.pack(">ii", *pos) + light)
                print(f"First lit packet {first_ms:.1f} ms, {len(columns)} columns {total_ms:.1f} ms, "
                      f"light SHA-256 {signature.hexdigest()}")
            finally:
                client.s.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--radius", type=int, choices=range(1, 9), default=4)
    measure(parser.parse_args().radius)
