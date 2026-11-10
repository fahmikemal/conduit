"""Real HTTP/2 gRPC echo fixture and client for the disposable soak host.

Dependencies are pinned in requirements.txt and supplied in ./python, or SOAK_PYTHON_LIB.
The client validates framed payloads, bidirectional replies and grpc-status trailers.
"""
import argparse
import asyncio
from collections import deque
import hashlib
import json
import os
from pathlib import Path
import ssl
import sys
import time

sys.path.insert(0, os.environ.get("SOAK_PYTHON_LIB", str(Path(__file__).parent / "python")))
from h2.config import H2Configuration
from h2.connection import H2Connection
from h2.events import (
    ConnectionTerminated, DataReceived, RequestReceived, ResponseReceived,
    StreamEnded, StreamReset, TrailersReceived,
)

MAX_MESSAGE = 2 * 1024 * 1024


def unpack_frames(buf):
    frames = []
    while len(buf) >= 5:
        if buf[0] != 0:
            raise ValueError("unexpected compressed gRPC frame")
        n = int.from_bytes(buf[1:5], "big")
        if n > MAX_MESSAGE:
            raise ValueError("oversized fixture message")
        if len(buf) < n + 5:
            break
        frames.append(bytes(buf[5:n + 5]))
        del buf[:n + 5]
    return frames


def atomic_json(path, value):
    p = Path(path)
    tmp = p.with_suffix(p.suffix + ".tmp")
    tmp.write_text(json.dumps(value, sort_keys=True) + "\n")
    tmp.replace(p)


async def publish(path, stats, stop):
    while not stop.is_set():
        atomic_json(path, dict(stats, sampled_at=time.time()))
        try:
            await asyncio.wait_for(stop.wait(), 5)
        except TimeoutError:
            pass
    atomic_json(path, dict(stats, sampled_at=time.time(), completed=True))


async def serve(args):
    stats = dict(connections=0, active_connections=0, grpc_streams=0,
                 messages=0, large_messages=0, resets=0, protocol_errors=0)

    async def connection(reader, writer):
        c = H2Connection(H2Configuration(client_side=False, header_encoding="utf-8"))
        c.initiate_connection()
        streams = {}
        stats["connections"] += 1
        stats["active_connections"] += 1

        def flush_streams():
            for sid, s in list(streams.items()):
                while s["pending"]:
                    packet = s["pending"][0]
                    n = min(c.local_flow_control_window(sid), c.max_outbound_frame_size,
                            len(packet) - s["offset"])
                    if n <= 0:
                        break
                    c.send_data(sid, packet[s["offset"]:s["offset"] + n])
                    s["offset"] += n
                    if s["offset"] == len(packet):
                        s["pending"].popleft()
                        s["offset"] = 0
                if s["ended"] and not s["pending"]:
                    status = "13" if s["buf"] else s["status"]
                    c.send_headers(sid, [("grpc-status", status)], end_stream=True)
                    del streams[sid]

        try:
            writer.write(c.data_to_send())
            await writer.drain()
            while data := await reader.read(65536):
                terminated = False
                for event in c.receive_data(data):
                    sid = getattr(event, "stream_id", None)
                    if isinstance(event, RequestReceived):
                        headers = dict(event.headers)
                        if headers.get(":path") == "/health":
                            c.send_headers(sid, [(":status", "200"), ("content-length", "2")])
                            c.send_data(sid, b"ok", end_stream=True)
                        else:
                            if headers.get(":path") != "/soak.Echo/Bidi":
                                raise ValueError("unknown gRPC method")
                            status = headers.get("x-soak-status", "0")
                            if status not in ("0", "7"):
                                raise ValueError("unknown expected grpc-status")
                            streams[sid] = dict(buf=bytearray(), pending=deque(), offset=0,
                                                ended=False, status=status)
                            stats["grpc_streams"] += 1
                            c.send_headers(sid, [(":status", "200"),
                                                ("content-type", "application/grpc")])
                    elif isinstance(event, DataReceived):
                        c.acknowledge_received_data(event.flow_controlled_length, sid)
                        if sid in streams:
                            s = streams[sid]
                            s["buf"].extend(event.data)
                            for message in unpack_frames(s["buf"]):
                                s["pending"].append(b"\0" + len(message).to_bytes(4, "big") + message)
                                stats["messages"] += 1
                                stats["large_messages"] += len(message) == MAX_MESSAGE
                    elif isinstance(event, StreamEnded) and sid in streams:
                        streams[sid]["ended"] = True
                    elif isinstance(event, StreamReset):
                        streams.pop(sid, None)
                        stats["resets"] += 1
                    elif isinstance(event, ConnectionTerminated):
                        terminated = True
                if terminated:
                    break
                flush_streams()
                writer.write(c.data_to_send())
                await writer.drain()
        except (ConnectionError, asyncio.IncompleteReadError):
            pass
        except Exception as exc:
            stats["protocol_errors"] += 1
            print(f"{time.time():.3f} origin {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        finally:
            stats["active_connections"] -= 1
            writer.close()
            try:
                await writer.wait_closed()
            except ConnectionError:
                pass

    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(args.cert, args.key)
    tls.set_alpn_protocols(["h2"])
    plain = await asyncio.start_server(connection, "127.0.0.1", args.h2c_port)
    secure = await asyncio.start_server(connection, "127.0.0.1", args.tls_port, ssl=tls)
    stop = asyncio.Event()
    async with plain, secure:
        await publish(args.stats, stats, stop)


class Client:
    def __init__(self, reader, writer):
        self.reader, self.writer = reader, writer
        self.c = H2Connection(H2Configuration(client_side=True, header_encoding="utf-8"))
        self.c.initiate_connection()
        self.sid = None
        self.expected = deque()
        self.buf = bytearray()
        self.received = 0
        self.sequence = 0
        self.trailers = None
        self.ended = False
        self.request_ended = False
        self.early_reply = False

    async def flush(self):
        self.writer.write(self.c.data_to_send())
        await self.writer.drain()

    async def receive(self):
        # Deliver GOAWAY before parsing another frame. hyper-h2 closes its state
        # machine on GOAWAY; a coalesced shutdown PING would otherwise mask that
        # event with ProtocolError before receive_data returns its event list.
        header = await asyncio.wait_for(self.reader.readexactly(9), 12)
        body = await asyncio.wait_for(
            self.reader.readexactly(int.from_bytes(header[:3], "big")), 12)
        for event in self.c.receive_data(header + body):
            if isinstance(event, ConnectionTerminated):
                raise ConnectionError(f"GOAWAY {event.error_code}")
            if isinstance(event, StreamReset):
                raise ConnectionError(f"RST_STREAM {event.error_code}")
            if getattr(event, "stream_id", None) != self.sid:
                continue
            if isinstance(event, ResponseReceived):
                h = dict(event.headers)
                if h.get(":status") != "200" or not h.get("content-type", "").startswith("application/grpc"):
                    raise ValueError(f"invalid gRPC response headers {h}")
            elif isinstance(event, DataReceived):
                self.c.acknowledge_received_data(event.flow_controlled_length, event.stream_id)
                self.buf.extend(event.data)
                for message in unpack_frames(self.buf):
                    if not self.expected or self.expected.popleft() != hashlib.sha256(message).digest():
                        raise ValueError("gRPC payload hash/order mismatch")
                    self.received += 1
                    if not self.request_ended:
                        self.early_reply = True
            elif isinstance(event, TrailersReceived):
                self.trailers = dict(event.headers)
            elif isinstance(event, StreamEnded):
                self.ended = True
        await self.flush()

    async def send_message(self, message):
        self.expected.append(hashlib.sha256(message).digest())
        target = self.received + 1
        packet = b"\0" + len(message).to_bytes(4, "big") + message
        offset = 0
        while offset < len(packet):
            n = min(self.c.local_flow_control_window(self.sid), self.c.max_outbound_frame_size,
                    len(packet) - offset)
            if n <= 0:
                await self.receive()
                continue
            self.c.send_data(self.sid, packet[offset:offset + n])
            offset += n
            await self.flush()
        while self.received < target:
            await self.receive()

    async def rpc(self, host, status, until, args, stats, worker):
        self.sid = self.c.get_next_available_stream_id()
        self.buf.clear()
        self.expected.clear()
        self.trailers = None
        self.ended = self.request_ended = self.early_reply = False
        self.c.send_headers(self.sid, [(":method", "POST"), (":scheme", "https"),
            (":authority", host), (":path", "/soak.Echo/Bidi"),
            ("content-type", "application/grpc"), ("te", "trailers"),
            ("x-soak-status", status)])
        await self.flush()
        sent = 0
        while sent < args.messages_per_rpc and time.time() < until - 1:
            self.sequence += 1
            sequence = self.sequence
            size = MAX_MESSAGE if sequence % args.large_every == 0 else 1024
            seed = f"{worker}:{sequence}:".encode()
            message = (seed * ((size + len(seed) - 1) // len(seed)))[:size]
            await self.send_message(message)
            stats["messages"] += 1
            stats["bytes"] += size
            stats["large_messages"] += size == MAX_MESSAGE
            sent += 1
            await asyncio.sleep(args.interval)
        self.request_ended = True
        self.c.end_stream(self.sid)
        await self.flush()
        while not self.ended:
            await self.receive()
        if self.buf or self.expected or self.trailers is None or self.trailers.get("grpc-status") != status:
            raise ValueError(f"invalid/truncated gRPC reply or trailers {self.trailers}")
        if sent and not self.early_reply:
            raise ValueError("no response payload before request EOF")
        stats["rpcs"] += 1
        stats["early_responses"] += bool(sent)
        stats["status_7"] += status == "7"


async def client(args):
    stats = dict(messages=0, bytes=0, large_messages=0, rpcs=0, early_responses=0,
                 status_7=0, errors=0, connections=0)
    log = open(args.events, "a", buffering=1)
    stop = asyncio.Event()

    async def worker(host):
        count = 0
        tls = ssl.create_default_context()
        tls.check_hostname = False
        tls.verify_mode = ssl.CERT_NONE  # disposable loopback fixture certificate
        tls.set_alpn_protocols(["h2"])
        while time.time() < args.until - 2:
            writer = None
            try:
                reader, writer = await asyncio.wait_for(asyncio.open_connection(
                    "127.0.0.1", args.https_port, ssl=tls, server_hostname=host), 10)
                if writer.get_extra_info("ssl_object").selected_alpn_protocol() != "h2":
                    raise ValueError("frontend did not negotiate HTTP/2")
                stats["connections"] += 1
                c = Client(reader, writer)
                await c.flush()
                while time.time() < args.until - 2:
                    count += 1
                    await c.rpc(host, "7" if count % 7 == 0 else "0", args.until,
                                args, stats, host)
            except Exception as exc:
                stats["errors"] += 1
                log.write(f"{time.time():.3f} {host} {type(exc).__name__}: {exc}\n")
                await asyncio.sleep(1)
            finally:
                if writer:
                    writer.close()
                    try:
                        await writer.wait_closed()
                    except (ConnectionError, ssl.SSLError):
                        pass

    task = asyncio.create_task(publish(args.stats, stats, stop))
    try:
        await asyncio.gather(*(worker(host) for host in args.hosts))
    finally:
        stop.set()
        await task
        log.close()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    commands = p.add_subparsers(dest="mode", required=True)
    s = commands.add_parser("server")
    s.add_argument("--h2c-port", type=int, default=9101)
    s.add_argument("--tls-port", type=int, default=9102)
    s.add_argument("--cert", required=True)
    s.add_argument("--key", required=True)
    s.add_argument("--stats", required=True)
    c = commands.add_parser("client")
    c.add_argument("--https-port", type=int, default=8443)
    c.add_argument("--until", type=float, required=True)
    c.add_argument("--stats", required=True)
    c.add_argument("--events", required=True)
    c.add_argument("--hosts", nargs="+", default=["grpc.soak.test", "grpctls.soak.test"])
    c.add_argument("--messages-per-rpc", type=int, default=90)
    c.add_argument("--interval", type=float, default=0.5)
    c.add_argument("--large-every", type=int, default=120)
    args = p.parse_args()
    asyncio.run(serve(args) if args.mode == "server" else client(args))


if __name__ == "__main__":
    main()
