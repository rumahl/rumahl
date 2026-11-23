#!/usr/bin/env python3
"""HTTPS acceptance checks against a running rumahl OS device.

The device certificate is signed by the device-local CA. Point --ca at that CA
(``/var/lib/rumahl-ca/ca.pem``, readable on the console) or use --insecure for a
first smoke test. Passwords and session cookies are never printed.
"""
import argparse
import getpass
import http.cookiejar
import json
import shutil
import ssl
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request

RESULTS = []


def check(name, condition, detail=""):
    RESULTS.append((name, bool(condition), detail))
    print(f"{'PASS' if condition else 'FAIL'}  {name}{(': ' + detail) if detail else ''}")
    return bool(condition)


def request(opener, url, method="GET", data=None, headers=None):
    req = urllib.request.Request(url, data=data, method=method, headers=headers or {})
    try:
        with opener.open(req, timeout=20) as response:
            return response.status, dict(response.headers), response.read()
    except urllib.error.HTTPError as error:
        return error.code, dict(error.headers), error.read()


def tls_sans(origin):
    if not shutil.which("openssl"):
        return None
    split = urllib.parse.urlsplit(origin)
    host = split.hostname
    port = split.port or 443
    try:
        output = subprocess.run(
            ["openssl", "s_client", "-connect", f"{host}:{port}", "-servername", host],
            input=b"", capture_output=True, timeout=20, check=False,
        ).stdout
        chain = subprocess.run(
            ["openssl", "x509", "-noout", "-ext", "subjectAltName"],
            input=output, capture_output=True, timeout=20, check=False,
        ).stdout.decode("utf-8", "replace")
        return " ".join(chain.split())
    except (OSError, subprocess.SubprocessError):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--origin", required=True, help="device origin, e.g. https://192.168.1.50")
    parser.add_argument("--ca", help="device CA certificate (PEM)")
    parser.add_argument("--insecure", action="store_true", help="skip TLS verification (smoke test only)")
    parser.add_argument("--username", default="alice")
    parser.add_argument("--password", help="prompted if omitted")
    args = parser.parse_args()

    origin = args.origin.rstrip("/")
    if not origin.startswith("https://"):
        parser.error("--origin must start with https://")
    if not args.insecure and not args.ca:
        parser.error("pass --ca /path/to/ca.pem or --insecure for the first test")
    password = args.password or getpass.getpass("Device password: ")

    if args.insecure:
        context = ssl.create_default_context()
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
    else:
        context = ssl.create_default_context(cafile=args.ca)

    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(
        urllib.request.HTTPCookieProcessor(jar),
        urllib.request.HTTPSHandler(context=context),
    )

    status, _, body = request(opener, origin + "/login")
    check("login page reachable", status == 200 and b"<form" in body, f"HTTP {status}")

    status, _, body = request(opener, origin + "/recovery")
    check("recovery page reachable", status == 200, f"HTTP {status}")

    status, _, body = request(opener, origin + "/api/v1/device/addresses")
    addresses = []
    if status == 200:
        try:
            addresses = json.loads(body).get("addresses", [])
        except json.JSONDecodeError:
            addresses = []
    check("device addresses published", status == 200 and bool(addresses), ", ".join(addresses))

    form = urllib.parse.urlencode({"username": args.username, "password": password}).encode()
    status, _, _ = request(
        opener,
        origin + "/login",
        "POST",
        form,
        {"Content-Type": "application/x-www-form-urlencoded", "Origin": origin},
    )
    session = any(cookie.name == "__Host-rumahl_session" for cookie in jar)
    check("login accepted", summary_status(status) and session, f"HTTP {status}")

    status, _, body = request(opener, origin + "/api/v1/shell/snapshot")
    display = None
    if status == 200:
        try:
            display = json.loads(body).get("user", {}).get("displayName")
        except json.JSONDecodeError:
            display = None
    check("authenticated snapshot", status == 200 and bool(display), f"HTTP {status}, user={display!r}")

    status, _, body = request(opener, origin + "/")
    check("shell renders", status == 200 and len(body) > 1024, f"HTTP {status}, {len(body)} bytes")

    sans = tls_sans(origin)
    if sans:
        host = urllib.parse.urlsplit(origin).hostname
        check("certificate covers the origin host", host in sans, sans)

    failed = [name for name, ok, _ in RESULTS if not ok]
    print()
    if failed:
        print(f"{len(failed)} check(s) failed: {', '.join(failed)}")
        return 1
    print(f"All {len(RESULTS)} checks passed.")
    return 0


def summary_status(status):
    # A successful login is followed to the shell (200) or observed as a 303.
    return status in (200, 303)


if __name__ == "__main__":
    sys.exit(main())
