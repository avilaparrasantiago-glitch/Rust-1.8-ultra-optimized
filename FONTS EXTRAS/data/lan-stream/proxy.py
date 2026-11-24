import asyncio
import contextlib
import hashlib
import hmac
import ipaddress
import os
import re
import time
import uuid

HOST = "192.168.1.150"
LAN = ipaddress.ip_network("192.168.1.0/24")
KEY = os.environ["CORAL_LAN_KEY"].encode()

def var(n):
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out) + bytes([n])

async def frame(reader):
    size = 0
    for i in range(3):
        b = (await reader.readexactly(1))[0]
        size |= (b & 127) << (7*i)
        if b < 128:
            if not 0 < size <= 2048:
                raise ValueError()
            return await reader.readexactly(size)
    raise ValueError()

def send(writer, body):
    writer.write(var(len(body)) + body)

async def copy(reader, writer):
    while data := await reader.read(32768):
        writer.write(data)
        await writer.drain()

async def client(reader, writer):
    backend = None
    tasks = []
    try:
        ip = writer.get_extra_info("peername")[0]
        if ipaddress.ip_address(ip) not in LAN:
            return
        async with asyncio.timeout(10):
            hello = await frame(reader)
            if hello[0] != 0 or hello[-1] not in (1, 2):
                return
            login = None
            if hello[-1] == 2:
                if hello[1] != 47:
                    return
                login = await frame(reader)
                if len(login) < 3 or login[0] != 0 or login[1] != len(login)-2 or not re.fullmatch(rb"[A-Za-z0-9_]{1,16}", login[2:]):
                    return
                name = login[2:].decode()
                uid = uuid.UUID(bytes=hashlib.md5(b"OfflinePlayer:" + login[2:]).digest(), version=3)
                claims = "\0".join(("Coral47/1", name, str(uid), ip, str(int(time.time())), str(uuid.uuid4()))).encode()
                host = claims + b"\0" + hmac.new(KEY, claims, hashlib.sha256).hexdigest().encode()
                hello = bytes([0,47]) + var(len(host)) + host + bytes.fromhex("63dd02")
            upstream, backend = await asyncio.open_connection("127.0.0.1", 25568)
            send(backend, hello)
            if login:
                send(backend, login)
            await backend.drain()
        tasks = [asyncio.create_task(copy(reader, backend)), asyncio.create_task(copy(upstream, writer))]
        await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
    except (OSError, ValueError, IndexError, asyncio.IncompleteReadError, TimeoutError):
        pass
    finally:
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        for stream in (writer, backend):
            if stream:
                stream.close()
                with contextlib.suppress(OSError):
                    await stream.wait_closed()

async def main():
    async with await asyncio.start_server(client, HOST, 25567) as server:
        await server.serve_forever()

asyncio.run(main())
