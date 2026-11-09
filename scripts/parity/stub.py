#!/usr/bin/env python3
"""Echo upstream for the parity test: listens on every port given and answers each request
with JSON describing what it received (port, method, target, headers, body length), so the
requests NPM and conduit forward can be compared.

    stub.py PORT [PORT...]
"""

import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Echo(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def handle_one_request(self):
        try:
            super().handle_one_request()
        except (ConnectionError, TimeoutError):
            self.close_connection = True

    def answer(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        seen = {
            "port": self.server.server_address[1],
            "method": self.command,
            "target": self.path,
            "headers": [[k.lower(), v] for k, v in self.headers.items()],
            "body_len": len(body),
        }
        out = json.dumps(seen).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(out)

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = do_OPTIONS = do_HEAD = answer

    def log_message(self, *args):
        pass


def main():
    servers = [ThreadingHTTPServer(("0.0.0.0", int(p)), Echo) for p in sys.argv[1:]]
    for s in servers[1:]:
        threading.Thread(target=s.serve_forever, daemon=True).start()
    print(f"stub listening on {len(servers)} ports", flush=True)
    servers[0].serve_forever()


if __name__ == "__main__":
    main()
