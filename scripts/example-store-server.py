#!/usr/bin/env python3
"""Small local HTTPS server for the generated example App Store."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import ssl


class Handler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()

    def log_message(self, format, *args):
        print(f"[example-store] {self.address_string()} {format % args}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--port", type=int, default=8787)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--tls-cert")
    parser.add_argument("--tls-key")
    parser.add_argument("--http", action="store_true", help="Serve plain HTTP (not recommended)")
    arguments = parser.parse_args()
    root = Path(arguments.root).resolve()
    if not (root / "site" / "store.json").is_file():
        raise SystemExit(f"{root} does not contain a generated example store")
    if not arguments.http and not (arguments.tls_cert and arguments.tls_key):
        raise SystemExit(
            "The example store uses HTTPS. Pass --tls-cert and --tls-key, "
            "or run scripts/dev-example-store.sh."
        )

    server = ThreadingHTTPServer(
        (arguments.host, arguments.port), partial(Handler, directory=str(root))
    )
    scheme = "http" if arguments.http else "https"
    if scheme == "https":
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(arguments.tls_cert, arguments.tls_key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
    print(f"Example App Store on {scheme}://{arguments.host}:{arguments.port}")
    print(f"Website index: {scheme}://{arguments.host}:{arguments.port}/site/store.json")
    print(f"GitHub raw base: {scheme}://{arguments.host}:{arguments.port}/github")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
