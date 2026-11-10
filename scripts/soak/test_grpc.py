"""Focused fixture regressions; run with the pinned SOAK_PYTHON_LIB installed."""
import asyncio
import hashlib
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("soak_grpc", Path(__file__).with_name("grpc.py"))
grpc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grpc)
from hyperframe.frame import PingFrame


class Writer:
    def write(self, data):
        pass

    async def drain(self):
        pass


class ClientTests(unittest.IsolatedAsyncioTestCase):
    def pair(self):
        reader = asyncio.StreamReader()
        client = grpc.Client(reader, Writer())
        server = grpc.H2Connection(grpc.H2Configuration(client_side=False))
        server.initiate_connection()
        server.receive_data(client.c.data_to_send())
        return reader, client, server

    async def test_coalesced_goaway_ping_preserves_shutdown_event(self):
        reader, client, server = self.pair()
        server.close_connection(error_code=0)
        ping = PingFrame(0)
        ping.opaque_data = b"shutdown"
        reader.feed_data(server.data_to_send() + ping.serialize())
        reader.feed_eof()
        with self.assertRaisesRegex(ConnectionError, "GOAWAY 0"):
            for _ in range(10):
                await client.receive()

    async def test_corrupted_echo_still_fails(self):
        reader, client, server = self.pair()
        client.sid = 1
        client.c.send_headers(1, [(":method", "POST"), (":scheme", "https"),
            (":authority", "grpc.soak.test"), (":path", "/soak.Echo/Bidi")])
        server.receive_data(client.c.data_to_send())
        client.expected.append(hashlib.sha256(b"correct").digest())
        server.send_headers(1, [(":status", "200"), ("content-type", "application/grpc")])
        server.send_data(1, b"\0\0\0\0\5wrong")
        reader.feed_data(server.data_to_send())
        reader.feed_eof()
        with self.assertRaisesRegex(ValueError, "payload hash/order mismatch"):
            for _ in range(10):
                await client.receive()


if __name__ == "__main__":
    unittest.main()
