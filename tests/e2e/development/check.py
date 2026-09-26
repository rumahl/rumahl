#!/usr/bin/env python3
"""Exercise the real dev server in a disposable source copy; never edit the checkout."""
import argparse
import base64
import contextlib
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import ssl
import subprocess
import tempfile
import time
import urllib.parse

REPO = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get("RUMAHL_SERVICE_BIN", REPO / "target/debug/rumahl-platform-service")).resolve()
NODE = os.environ.get("NODE_BIN", "node")


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=12)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise RuntimeError("developer server did not shut down cleanly")


def frontend_copy(destination):
    original = REPO / "frontend"
    shutil.copytree(original, destination, ignore=shutil.ignore_patterns("node_modules", "dist", "coverage"))
    # Third-party dependencies are shared; workspace imports point to the copy.
    (destination / "node_modules").symlink_to(original / "node_modules", target_is_directory=True)
    for name in ["shell", "ssr"]:
        modules = destination / "packages" / name / "node_modules"
        modules.mkdir()
        for dependency in (original / "packages" / name / "node_modules").iterdir():
            if dependency.name == "@rumahl":
                scope = modules / dependency.name
                scope.mkdir()
                for package in dependency.iterdir():
                    (scope / package.name).symlink_to(destination / "packages" / package.name, target_is_directory=True)
            else:
                (modules / dependency.name).symlink_to(dependency.resolve(), target_is_directory=dependency.is_dir())


def run(browser=False):
    with tempfile.TemporaryDirectory(prefix="rumahl-dev-test-") as directory, contextlib.ExitStack() as stack:
        root = Path(directory)
        project = root / "project"
        frontend_copy(project / "frontend")
        state = root / "state"
        log = stack.enter_context((root / "dev.log").open("w+"))
        command = [NODE, str(project / "frontend/packages/ssr/dev/start.js"), "--binary", str(BINARY),
                   "--state-dir", str(state), "--port", "0", "--poll"]

        def launch():
            (state / "server.json").unlink(missing_ok=True)
            process = subprocess.Popen(command, stdout=log, stderr=log)
            stack.callback(stop, process)
            for _ in range(600):
                if process.poll() is not None:
                    raise RuntimeError("developer server exited during startup")
                if (state / "server.json").exists():
                    return process, json.loads((state / "server.json").read_text())
                time.sleep(0.1)
            raise RuntimeError("developer startup timed out")

        try:
            process, info = launch()
            origin = info["origin"]
            port = urllib.parse.urlsplit(origin).port
            context = ssl.create_default_context(cafile=info["certificate"])
            credentials = json.loads((state / "credentials.json").read_text())
            assert (state / "credentials.json").stat().st_mode & 0o777 == 0o600

            def request(path, method="GET", body=None, cookie=None, extra=None):
                headers = extra.copy() if extra else {}
                if cookie:
                    headers["Cookie"] = cookie
                if body is not None:
                    body = urllib.parse.urlencode(body)
                    headers["Content-Type"] = "application/x-www-form-urlencoded"
                connection = http.client.HTTPSConnection("localhost", port, context=context, timeout=20)
                try:
                    connection.request(method, path, body, headers)
                    response = connection.getresponse()
                    return response.status, dict(response.getheaders()), response.read()
                finally:
                    connection.close()

            assert request("/")[0] == 303
            assert request("/login")[1]["referrer-policy"] == "same-origin"
            assert request("/login", "POST", credentials, extra={"Origin": "null"})[0] == 403
            assert request("/login", "POST", credentials, extra={"Origin": "https://foreign.test"})[0] == 403
            code, headers, _ = request("/login", "POST", credentials, extra={"Origin": origin})
            assert code == 303
            cookie = headers["set-cookie"].split(";", 1)[0]
            assert "Secure" in headers["set-cookie"] and "HttpOnly" in headers["set-cookie"]
            code, headers, body = request("/", cookie=cookie)
            assert code == 200, code
            assert b'data-shell-ssr="1"' in body and b'/@vite/client' in body and b'/@react-refresh' in body
            assert info["buildId"].encode() in body
            assert credentials["password"].encode() not in body and cookie.encode() not in body
            assert b'RUMAHL_DEV_NONCE_PLACEHOLDER' not in body
            assert "'unsafe-inline'" not in headers["content-security-policy"]
            snapshot = json.loads(request("/api/v1/shell/snapshot", cookie=cookie)[2])
            assert snapshot["user"]["displayName"] == "Developer"
            assert request("/src/main.tsx")[0] == 200
            assert request("/src/main.tsx", extra={"Origin": "https://foreign.test"})[0] == 403
            assert request("/login", extra={"Host": "foreign.test"})[0] == 421
            assert request("/@fs/" + str(state / "credentials.json"))[0] in (403, 404)
            print("PASS: real login, TLS, SSR/build coherence and development origin/file boundaries", flush=True)

            # Register an actual React module with Vite, then observe a real HMR update.
            assert request("/src/App.tsx")[0] == 200
            client = request("/@vite/client")[2].decode()
            token = re.search(r'const wsToken = "([^"]+)"', client).group(1)
            ws = context.wrap_socket(socket.create_connection(("127.0.0.1", port)), server_hostname="localhost")
            stack.callback(ws.close)
            ws.settimeout(20)
            key = base64.b64encode(secrets.token_bytes(16)).decode()
            ws.sendall((f"GET /__dev/hmr?token={token} HTTP/1.1\r\nHost: localhost:{port}\r\n"
                        f"Origin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: vite-hmr\r\n\r\n").encode())
            handshake = b""
            while b"\r\n\r\n" not in handshake:
                part = ws.recv(1)
                assert part, "HMR handshake closed"
                handshake += part
            assert b"101 Switching Protocols" in handshake

            def receive_exact(length):
                data = b""
                while len(data) < length:
                    part = ws.recv(length - len(data))
                    assert part
                    data += part
                return data

            def event():
                header = receive_exact(2)
                assert header[0] & 0x0F == 1
                length = header[1] & 0x7F
                if length == 126:
                    length = int.from_bytes(receive_exact(2), "big")
                assert length < 65536
                return json.loads(receive_exact(length))

            assert event()["type"] == "connected"
            app = project / "frontend/packages/shell/src/App.tsx"
            app.write_text(app.read_text().replace('className="shell"', 'className="shell" data-dev-probe="changed"'))
            assert event()["type"] in ("update", "full-reload")
            for _ in range(100):
                body = request("/", cookie=cookie)[2]
                if b'data-dev-probe="changed"' in body:
                    break
                time.sleep(0.1)
            assert b'data-dev-probe="changed"' in body, "SSR did not reload the changed component"
            ws.close()
            print("PASS: authenticated HTTPS HMR connection and updated SSR without restarting", flush=True)

            if browser:
                subprocess.run([NODE, str(project / "frontend/tests/development-browser.mjs"), str(state)],
                               check=True, timeout=120)

            duplicate = subprocess.run(command, stdout=log, stderr=log, timeout=20)
            assert duplicate.returncode != 0 and process.poll() is None
            stop(process)
            assert not (state / "runner.lock").exists()
            process, info = launch()
            origin = info["origin"]
            port = urllib.parse.urlsplit(origin).port
            assert request("/", cookie=cookie)[0] == 200
            assert request("/logout", "POST", cookie=cookie, extra={"Origin": origin})[0] == 303
            assert request("/api/v1/shell/snapshot", cookie=cookie)[0] == 401
            print("PASS: single-runner lock, clean shutdown, persistent session and real logout", flush=True)
        except Exception:
            log.flush()
            log.seek(0)
            print(log.read()[-8000:])
            raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", action="store_true", help="also run installed Playwright Chromium")
    run(browser=parser.parse_args().browser)
