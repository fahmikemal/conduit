"""WebSocket side of the soak test, standard library only.

  ws.py server PORT
      Echo server: answers the WebSocket handshake and echoes every message.
  ws.py client HTTP_PORT HTTPS_PORT END_EPOCH STATS_CSV EVENTS_LOG
      Through conduit (Host ws.soak.test): 40 long-lived connections, half over TLS, each
      sending a message every second and checking the echo; plus a short-lived connection
      every 0.5 s (handshake, 3 messages, close). Every 30 s a line goes to STATS_CSV; every
      error or reconnect goes to EVENTS_LOG with its time.
"""

import asyncio
import base64
import hashlib
import os
import ssl
import struct
import sys
import time

GUID = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


async def read_frame(reader):
    b0, b1 = await reader.readexactly(2)
    n = b1 & 0x7F
    if n == 126:
        (n,) = struct.unpack("!H", await reader.readexactly(2))
    elif n == 127:
        (n,) = struct.unpack("!Q", await reader.readexactly(8))
    mask = await reader.readexactly(4) if b1 & 0x80 else None
    data = await reader.readexactly(n)
    if mask:
        data = bytes(c ^ mask[i % 4] for i, c in enumerate(data))
    return b0 & 0x0F, data


def frame(opcode, data, masked):
    head = bytes([0x80 | opcode])
    n = len(data)
    m = 0x80 if masked else 0
    if n < 126:
        head += bytes([m | n])
    elif n < 65536:
        head += bytes([m | 126]) + struct.pack("!H", n)
    else:
        head += bytes([m | 127]) + struct.pack("!Q", n)
    if not masked:
        return head + data
    key = os.urandom(4)
    return head + key + bytes(c ^ key[i % 4] for i, c in enumerate(data))


async def serve(reader, writer):
    try:
        head = await reader.readuntil(b"\r\n\r\n")
        lines = head.decode("latin-1").split("\r\n")
        headers = {}
        for line in lines[1:]:
            if ":" in line:
                k, v = line.split(":", 1)
                headers[k.strip().lower()] = v.strip()
        key = headers.get("sec-websocket-key")
        if not key:
            writer.write(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nok\n")
            await writer.drain()
            return
        accept = base64.b64encode(hashlib.sha1(key.encode() + GUID).digest())
        writer.write(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
            b"Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n"
        )
        await writer.drain()
        while True:
            op, data = await read_frame(reader)
            if op == 8:
                writer.write(frame(8, data[:2], False))
                await writer.drain()
                return
            if op == 9:
                writer.write(frame(10, data, False))
            elif op in (1, 2):
                writer.write(frame(op, data, False))
            await writer.drain()
    except (asyncio.IncompleteReadError, ConnectionError, asyncio.LimitOverrunError):
        pass
    finally:
        writer.close()


class Stats:
    def __init__(self, events):
        self.msgs = self.errors = self.reconnects = self.short_ok = self.short_err = 0
        self.open = 0
        self.events = open(events, "a", buffering=1)

    def error(self, what, e):
        self.events.write(f"{time.time():.3f} {what}: {type(e).__name__}: {e}\n")


async def connect(port, tls):
    ctx = None
    if tls:
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
    reader, writer = await asyncio.wait_for(
        asyncio.open_connection(
            "127.0.0.1", port, ssl=ctx, server_hostname="ws.soak.test" if tls else None
        ),
        5,
    )
    key = base64.b64encode(os.urandom(16))
    writer.write(
        b"GET /ws HTTP/1.1\r\nHost: ws.soak.test\r\nUpgrade: websocket\r\n"
        b"Connection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: "
        + key
        + b"\r\n\r\n"
    )
    await writer.drain()
    head = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 5)
    want = base64.b64encode(hashlib.sha1(key + GUID).digest())
    if not head.startswith(b"HTTP/1.1 101") or want not in head:
        writer.close()
        raise ConnectionError(f"handshake answered {head[:40]!r}")
    return reader, writer


async def echo(reader, writer, msg):
    writer.write(frame(1, msg, True))
    await writer.drain()
    op, data = await asyncio.wait_for(read_frame(reader), 5)
    if op != 1 or data != msg:
        raise ConnectionError(f"echo mismatch: sent {msg[:20]!r}, got {data[:20]!r}")


async def long_lived(i, port, tls, end, st):
    seq = 0
    while time.time() < end:
        try:
            reader, writer = await connect(port, tls)
        except Exception as e:
            st.errors += 1
            st.error(f"long {i} connect", e)
            await asyncio.sleep(1)
            continue
        st.open += 1
        try:
            while time.time() < end:
                seq += 1
                await echo(reader, writer, f"long {i} {seq}".encode() * 4)
                st.msgs += 1
                await asyncio.sleep(1)
            writer.write(frame(8, struct.pack("!H", 1000), True))
            await writer.drain()
        except Exception as e:
            st.errors += 1
            st.reconnects += 1
            st.error(f"long {i} {'tls' if tls else 'plain'}", e)
        finally:
            st.open -= 1
            writer.close()


async def short_lived(n, port, tls, st):
    try:
        reader, writer = await connect(port, tls)
        try:
            for k in range(3):
                await echo(reader, writer, f"short {n} {k}".encode())
            writer.write(frame(8, struct.pack("!H", 1000), True))
            await writer.drain()
            await asyncio.wait_for(read_frame(reader), 5)
        finally:
            writer.close()
        st.short_ok += 1
    except Exception as e:
        st.short_err += 1
        st.error(f"short {n}", e)


async def client(http_port, https_port, end, stats_csv, events):
    st = Stats(events)
    tasks = [
        asyncio.create_task(long_lived(i, https_port if i % 2 else http_port, i % 2 == 1, end, st))
        for i in range(40)
    ]

    async def shorts():
        n = 0
        while time.time() < end:
            n += 1
            asyncio.create_task(short_lived(n, https_port if n % 2 else http_port, n % 2 == 1, st))
            await asyncio.sleep(0.5)

    async def report():
        new = not os.path.exists(stats_csv) or os.path.getsize(stats_csv) == 0
        with open(stats_csv, "a", buffering=1) as f:
            if new:
                f.write("ts,open,msgs,errors,reconnects,short_ok,short_err\n")
            while time.time() < end:
                await asyncio.sleep(30)
                f.write(
                    f"{int(time.time())},{st.open},{st.msgs},{st.errors},{st.reconnects},"
                    f"{st.short_ok},{st.short_err}\n"
                )

    tasks += [asyncio.create_task(shorts()), asyncio.create_task(report())]
    await asyncio.gather(*tasks)


async def server(port):
    srv = await asyncio.start_server(serve, "127.0.0.1", port, limit=1 << 20)
    async with srv:
        await srv.serve_forever()


if __name__ == "__main__":
    if sys.argv[1] == "server":
        asyncio.run(server(int(sys.argv[2])))
    else:
        asyncio.run(
            client(int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), sys.argv[5], sys.argv[6])
        )
