#!/usr/bin/env python3
"""Real HTTPS gateway/SSR/process-restart acceptance test. No container mocks.

Build the Rust service and frontend first. Requires node, nginx and openssl.
Only loopback listeners and ephemeral state/certificates are used.
"""
import base64
import contextlib
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import socket
import ssl
import subprocess
import tempfile
import time
import urllib.parse

REPO = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get("RUMAHL_SERVICE_BIN", REPO / "target/debug/rumahl-platform-service")).resolve()
NGINX = os.environ.get("NGINX_BIN", "nginx")
NODE = os.environ.get("NODE_BIN", "node")


def wait_for(check, process):
    for _ in range(150):
        if process.poll() is not None:
            raise RuntimeError("service exited before becoming ready")
        try:
            if check():
                return
        except (OSError, http.client.HTTPException):
            pass
        time.sleep(0.05)
    raise RuntimeError("service did not become ready")


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def run():
    with tempfile.TemporaryDirectory(prefix="rumahl-shell-") as temporary, contextlib.ExitStack() as stack:
        root = Path(temporary)
        root.chmod(0o700)
        for name in ["state", "gateway", "renderer", "nginx"]:
            (root / name).mkdir(mode=0o700)
        log = stack.enter_context((root / "process.log").open("w+"))
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"https://localhost:{port}"
        key, cert = root / "key.pem", root / "cert.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                        "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost",
                        "-keyout", str(key), "-out", str(cert)], check=True, stdout=log, stderr=log)
        context = ssl.create_default_context(cafile=str(cert))
        blocklist = root / "blocked.txt"
        blocklist.write_text("this password is forbidden\n")
        env = dict(os.environ, RUMAHL_STATE_DIR=str(root / "state"), RUMAHL_PUBLIC_ORIGIN=origin,
                   RUMAHL_GATEWAY_SOCKET=str(root / "gateway/http.sock"), RUMAHL_GATEWAY_SOCKET_ACCESS="owner",
                   RUMAHL_SSR_SOCKET=str(root / "renderer/ssr.sock"), RUMAHL_SSR_SOCKET_ACCESS="owner",
                   RUMAHL_CLIENT_BUILD=str(REPO / "frontend/packages/shell/dist/build-id.json"),
                   RUMAHL_PASSWORD_BLOCKLIST=str(blocklist), RUMAHL_LOCALE="en", NODE_ENV="production")
        passwords = {name: secrets.token_urlsafe(30) for name in ["alice", "bob"]}
        for name, password in passwords.items():
            subprocess.run([str(BINARY), "provision-account", name, name.title(), "--password-stdin"],
                           input=password.encode(), env=env, check=True, stdout=log, stderr=log)

        def launch(command, **kwargs):
            process = subprocess.Popen(command, env=env, stdout=log, stderr=log, **kwargs)
            stack.callback(stop, process)
            return process

        # Use a fresh deployment tree with no node_modules to prove SSR is bundled.
        staged = root / "image"
        subprocess.run(["python3", str(REPO / "platform-buildroot/deployment/stage.py"), "--binary", str(BINARY),
                        "--output", str(staged)], check=True, stdout=log, stderr=log)
        renderer = launch([NODE, str(staged / "usr/share/rumahl/frontend/packages/ssr/server.js")])
        wait_for(lambda: (root / "renderer/ssr.sock").exists(), renderer)
        gateway = launch([str(BINARY), "serve"])
        wait_for(lambda: (root / "gateway/http.sock").exists(), gateway)
        # Adapt only deployment paths, hostname and loopback port in the shipped config.
        site = (REPO / "platform-buildroot/nginx/rumahl.conf").read_text()
        replacements = {
            "listen 443 ssl;": f"listen 127.0.0.1:{port} ssl;",
            "rumahl.home.arpa": "localhost",
            "/run/rumahl-platform/gateway.sock": str(root / "gateway/http.sock"),
            "/etc/rumahl/tls/fullchain.pem": str(cert),
            "/etc/rumahl/tls/key.pem": str(key),
            "/usr/share/rumahl/frontend/packages/shell/dist": str(staged / "usr/share/rumahl/frontend/packages/shell/dist"),
            "/etc/nginx/rumahl-proxy.inc": str(root / "proxy.inc"),
        }
        for old, new in replacements.items():
            site = site.replace(old, new)
        (root / "proxy.inc").write_text((REPO / "platform-buildroot/nginx/rumahl-proxy.inc").read_text())
        config = root / "nginx.conf"
        config.write_text(f'''daemon off;
master_process off;
pid {root}/nginx.pid;
error_log {root}/nginx-error.log warn;
events {{ worker_connections 64; }}
http {{
    types {{ text/css css; application/javascript js; }}
    client_body_temp_path {root}/nginx/body;
    proxy_temp_path {root}/nginx/proxy;
    fastcgi_temp_path {root}/nginx/fastcgi;
    uwsgi_temp_path {root}/nginx/uwsgi;
    scgi_temp_path {root}/nginx/scgi;
    {site}
}}
''')
        subprocess.run([NGINX, "-t", "-p", str(root), "-c", str(config)], check=True, stdout=log, stderr=log)
        edge = launch([NGINX, "-p", str(root), "-c", str(config)])

        def request(path, method="GET", body=None, cookie=None, request_origin=None):
            connection = http.client.HTTPSConnection("localhost", port, context=context, timeout=10)
            headers = {}
            if body is not None:
                headers["Content-Type"] = "application/x-www-form-urlencoded"
                body = urllib.parse.urlencode(body)
            if cookie:
                headers["Cookie"] = cookie
            if request_origin:
                headers["Origin"] = request_origin
            connection.request(method, path, body, headers)
            response = connection.getresponse()
            result = response.status, {key.lower(): value for key, value in response.getheaders()}, response.read()
            connection.close()
            return result

        try:
            wait_for(lambda: request("/login")[0] == 200, edge)
            assert request("/")[0] == 303
            assert request("/api/v1/shell/snapshot")[0] == 401
            assert request("/login", "POST", {"username": "alice", "password": passwords["alice"]}, request_origin="https://foreign.test")[0] == 403
            cookies = {}
            for name in passwords:
                status, headers, _ = request("/login", "POST", {"username": name, "password": passwords[name]}, request_origin=origin)
                assert status == 303
                cookie = headers["set-cookie"]
                assert all(flag in cookie for flag in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"])
                cookies[name] = cookie.split(";", 1)[0]
                status, headers, body = request("/", cookie=cookies[name])
                assert status == 200, f"shell returned {status}: {body[:200]!r}"
                assert name.title().encode() in body, "SSR omitted authenticated display name"
                assert headers["cache-control"] == "private, no-store"
                assert b'data-shell-ssr="1"' in body and b'id="rumahl-shell-snapshot"' in body
                assert passwords[name].encode() not in body and cookies[name].encode() not in body
                for asset in re.findall(rb'(?:src|href)="(/(?:assets|shell/themes)/[^\"]+)"', body):
                    status, asset_headers, _ = request(asset.decode())
                    assert status == 200 and "immutable" in asset_headers["cache-control"]
                snapshot = json.loads(request("/api/v1/shell/snapshot", cookie=cookies[name])[2])
                assert snapshot["user"]["displayName"] == name.title()
                assert snapshot["systemStatus"]["installedAppCount"] == 0
            print("PASS: trusted HTTPS, login, isolated SQLite users, SSR and immutable assets", flush=True)
            stop(gateway)
            (root / "gateway/http.sock").unlink()
            gateway = launch([str(BINARY), "serve"])
            wait_for(lambda: request("/api/v1/shell/snapshot", cookie=cookies["alice"])[0] == 200, gateway)
            # Real authenticated WebSocket: logout must notify an already open tab.
            ws = context.wrap_socket(socket.create_connection(("127.0.0.1", port)), server_hostname="localhost")
            stack.callback(ws.close)
            ws.settimeout(40)
            key = base64.b64encode(secrets.token_bytes(16)).decode()
            ws.sendall((f"GET /api/v1/shell/events HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: {origin}\r\n"
                        f"Cookie: {cookies['alice']}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
            handshake = b""
            while b"\r\n\r\n" not in handshake:
                handshake += ws.recv(1)
            assert b"101 Switching Protocols" in handshake
            assert request("/logout", "POST", cookie=cookies["alice"], request_origin=origin)[0] == 303
            def receive_exact(length):
                result = b""
                while len(result) < length:
                    part = ws.recv(length - len(result))
                    assert part, "WebSocket closed before revocation notification"
                    result += part
                return result
            header = receive_exact(2)
            assert header[0] & 0x0F == 1 and header[1] & 0x80 == 0
            length = header[1] & 0x7F
            if length == 126:
                length = int.from_bytes(receive_exact(2), "big")
            assert length < 1024
            assert b"session_revoked" in receive_exact(length)
            stop(gateway)
            (root / "gateway/http.sock").unlink()
            gateway = launch([str(BINARY), "serve"])
            wait_for(lambda: request("/api/v1/shell/snapshot", cookie=cookies["bob"])[0] == 200, gateway)
            assert request("/api/v1/shell/snapshot", cookie=cookies["alice"])[0] == 401
            print("PASS: process restart preserves sessions; logout survives restart and notifies open tabs", flush=True)
            stop(renderer)
            assert request("/", cookie=cookies["bob"])[0] == 503
            assert request("/recovery")[0] == 200
            assert request("/login")[0] == 200
            print("PASS: renderer failure leaves login and independent recovery available", flush=True)
        except Exception:
            # Process logs contain no passwords, cookies or HTTP request URLs.
            log.flush()
            log.seek(0)
            print(log.read()[-6000:])
            raise


if __name__ == "__main__":
    run()
