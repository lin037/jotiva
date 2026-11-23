"""Verify the native HTTP client against a loopback server with a synthetic note."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
from threading import Event, Thread

PROJECT = Path(__file__).resolve().parents[1]
PAYLOAD = {"state": "测试便签", "questions": {"destination": {"type": "choice"}}}
REPLY = b'{"answers":{"destination":{"type":"choice","choice":"other"}}}'


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        content = self.rfile.read(int(self.headers["Content-Length"]))
        self.server.requests.append((
            self.path, self.headers["Authorization"], self.headers["Content-Type"], json.loads(content),
        ))
        mode = self.path.rsplit("/", 1)[-1]
        if mode == "timeout":
            self.server.done.wait(5)
            return
        status, data = {
            "ok": (200, REPLY), "redirect": (307, b""), "nul": (200, REPLY + b"\0extra"),
            "oversize": (200, b"x" * (128 * 1024 + 1)),
        }.get(mode, (int(mode) if mode.isdigit() else 500, b""))
        self.send_response(status)
        self.send_header("Content-Length", str(len(data)))
        if mode == "redirect":
            self.send_header("Location", f"http://127.0.0.1:{self.server.server_port}/must-not-follow")
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_):
        pass


with tempfile.TemporaryDirectory(prefix="jot-http-qa-") as directory:
    temp = Path(directory)
    def compile_probe(name):
        binary = temp / name
        subprocess.run([
            "clang", "-fobjc-arc", "-framework", "AppKit", "-framework", "Security",
            "-framework", "LocalAuthentication", "-lcurl",
            str(PROJECT / f"qa/macos/{name}.m"), "-o", str(binary),
        ], check=True)
        return binary
    subprocess.run([compile_probe("filing-key")], check=True, timeout=4)
    client = compile_probe("filing-http")
    class IPv6Server(ThreadingHTTPServer):
        address_family = socket.AF_INET6

    cases = [("ok", 200), ("401", 401), ("403", 403), ("429", 429), ("503", 503),
             ("timeout", -2), ("oversize", -3), ("nul", -3), ("redirect", 307)]

    def check(server, origin, cases, *, tls=False):
        if tls:
            cert, key = temp / "test-cert.pem", temp / "test-key.pem"
            subprocess.run(["/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-days", "1", "-subj", "/CN=127.0.0.1", "-keyout", key, "-out", cert],
                           check=True, capture_output=True)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            server.socket = context.wrap_socket(server.socket, server_side=True)
        server.requests, server.done = [], Event()
        Thread(target=server.serve_forever, daemon=True).start()
        # A loopback credential must reach the local service even with a proxy and no bypass list.
        env = {**os.environ, "no_proxy": "", "NO_PROXY": "", "http_proxy": "http://127.0.0.1:1",
               "https_proxy": "http://127.0.0.1:1", "ALL_PROXY": "http://127.0.0.1:1"}
        try:
            for mode, status in cases:
                url = f"{origin}:{server.server_port}/v1/systemone/{mode}"
                result = subprocess.run([client, url, json.dumps(PAYLOAD)], env=env,
                                        check=True, capture_output=True, timeout=4)
                code, _, body = result.stdout.partition(b"\n")
                assert int(code) == status, (mode, result)
                assert body == (REPLY if status == 200 else b""), mode
        finally:
            server.done.set()
            server.shutdown()
        expected = [] if tls else [
            ("/v1/systemone/" + mode, "Bearer qa-fixture-key", "application/json", PAYLOAD)
            for mode, _ in cases
        ]
        assert server.requests == expected, server.requests

    with ThreadingHTTPServer(("127.0.0.1", 0), Mock) as server:
        check(server, "http://127.0.0.1", cases)
    with IPv6Server(("::1", 0), Mock) as server:
        check(server, "http://[::1]", [("ok", 200)])
    with ThreadingHTTPServer(("127.0.0.1", 0), Mock) as server:
        check(server, "https://127.0.0.1", [("ok", -4)], tls=True)
    print("Native client passed credential binding, HTTP errors, timeout, response limits, redirects, IPv4/IPv6 proxy bypass and rejection of an untrusted TLS certificate before POST.")
