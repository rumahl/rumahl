#!/usr/bin/env python3
"""Build, sign and index the twenty example apps for the local App Store."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]


def run(command, **kwargs):
    subprocess.run(command, check=True, **kwargs)


def target_directory():
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version=1"],
        cwd=REPO, check=True, capture_output=True, text=True,
    )
    return Path(json.loads(result.stdout)["target_directory"])


def merge_trust_store(shared: Path, generated: Path):
    source = json.loads(generated.read_text())
    if shared.exists():
        document = json.loads(shared.read_text())
        if document.get("formatVersion") != 1 or not isinstance(document.get("keys"), list):
            raise SystemExit(f"{shared} is not a rumahl trust store")
    else:
        document = {"formatVersion": 1, "keys": []}
    known = {key.get("keyId") for key in document["keys"]}
    for key in source["keys"]:
        if key.get("keyId") not in known:
            document["keys"].append(key)
    shared.parent.mkdir(parents=True, exist_ok=True)
    shared.parent.chmod(0o700)
    shared.write_text(json.dumps(document, indent=2) + "\n")
    shared.chmod(0o600)


def ensure_tls(state: Path):
    tls = state / "example-store-tls"
    certificate = tls / "cert.pem"
    key = tls / "key.pem"
    ca = tls / "ca.pem"
    if certificate.is_file() and key.is_file() and ca.is_file():
        return certificate, key, ca
    tls.mkdir(parents=True, exist_ok=True)
    tls.chmod(0o700)
    ca_key = tls / "ca-key.pem"
    request = tls / "server.csr"
    extensions = tls / "server.ext"
    extensions.write_text(
        "basicConstraints=critical,CA:FALSE\n"
        "keyUsage=critical,digitalSignature,keyEncipherment\n"
        "extendedKeyUsage=serverAuth\n"
        "subjectAltName=DNS:localhost,IP:127.0.0.1\n"
    )
    commands = [
        ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "365",
         "-subj", "/CN=rumahl Example Store Local CA",
         "-addext", "basicConstraints=critical,CA:TRUE",
         "-addext", "keyUsage=critical,keyCertSign,cRLSign",
         "-keyout", str(ca_key), "-out", str(ca)],
        ["openssl", "req", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=localhost",
         "-keyout", str(key), "-out", str(request)],
        ["openssl", "x509", "-req", "-in", str(request), "-CA", str(ca),
         "-CAkey", str(ca_key), "-CAcreateserial", "-days", "365",
         "-extfile", str(extensions), "-out", str(certificate)],
    ]
    try:
        for command in commands:
            result = subprocess.run(command, capture_output=True, text=True)
            if result.returncode != 0:
                raise SystemExit(f"Could not create local HTTPS certificate: {result.stderr.strip()}")
    except FileNotFoundError:
        raise SystemExit("openssl is required to create the local HTTPS test certificate")
    finally:
        for temporary in (ca_key, request, extensions, tls / "ca.srl"):
            temporary.unlink(missing_ok=True)
    ca.chmod(0o600)
    certificate.chmod(0o600)
    key.chmod(0o600)
    return certificate, key, ca


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", default=str(REPO / ".rumahl-dev"))
    parser.add_argument("--output", help="Generated server root (default: STATE/example-store)")
    parser.add_argument("--binary", help="Path to rumahl-package")
    arguments = parser.parse_args()

    state = Path(arguments.state_dir).resolve()
    output = Path(arguments.output or state / "example-store").resolve()
    source = REPO / "examples" / "app-store"
    apps_root = source / "apps"
    catalog = json.loads((source / "catalog.json").read_text())
    entries = {entry["slug"]: entry for entry in catalog["apps"]}
    slugs = sorted(path.name for path in apps_root.iterdir() if path.is_dir())
    if set(slugs) != set(entries):
        raise SystemExit("catalog.json and examples/app-store/apps are out of sync")

    if arguments.binary:
        binary = Path(arguments.binary)
    else:
        print("Building rumahl-package…")
        run(["cargo", "build", "--locked", "-p", "rumahl-app-packages", "--bin", "rumahl-package"], cwd=REPO)
        binary = target_directory() / "debug" / "rumahl-package"

    keys = state / "keys"
    keys.mkdir(parents=True, exist_ok=True)
    keys.chmod(0o700)
    keystore = keys / "example-store.json"
    trust_store = keys / "trust-store.json"
    if not keystore.exists():
        print(f"Generating example-store signing key in {keystore}…")
        with tempfile.TemporaryDirectory() as temporary:
            generated = Path(temporary) / "trust-store.json"
            run([
                str(binary), "keygen", "--key-id", "example-store",
                "--publisher", "com.rumahl.examples",
                "--keystore", str(keystore), "--trust-store", str(generated),
            ])
            merge_trust_store(trust_store, generated)
    elif not trust_store.exists():
        raise SystemExit(
            f"{keystore} exists but {trust_store} is missing; remove the keystore to generate a new key"
        )

    certificate, key, ca = ensure_tls(state)

    if output.exists():
        shutil.rmtree(output)
    site = output / "site"
    github = output / "github" / "example" / "apps" / "main"
    package_root = site / "packages"
    (package_root).mkdir(parents=True)
    github.mkdir(parents=True)

    index = []
    for slug in slugs:
        entry = entries[slug]
        source_app = apps_root / slug
        manifest = json.loads((source_app / "manifest.json").read_text())
        app = manifest["app"]
        destination = package_root / slug
        destination.mkdir()
        shutil.copytree(source_app / "frontend", destination / "frontend")
        run([
            str(binary), "sign", "--package", str(destination),
            "--manifest", str(source_app / "manifest.json"),
            "--keystore", str(keystore),
        ])
        shutil.copytree(destination, github / "packages" / slug)
        index.append({
            "id": app["appId"],
            "title": entry["title"],
            "version": app["version"],
            "description": entry["description"],
            "publisher": manifest["publisherId"],
            "packageUrl": f"packages/{slug}",
        })

    document = {
        "formatVersion": 1,
        "name": catalog["name"],
        "apps": index,
    }
    (site / "store.json").write_text(json.dumps(document, indent=2) + "\n")
    (github / "store.json").write_text(json.dumps(document, indent=2) + "\n")

    total_bytes = sum(path.stat().st_size for path in output.rglob("*") if path.is_file())
    print(
        f"Generated {len(index)} signed example apps in {output} "
        f"({total_bytes / 1024:.0f} KiB)"
    )
    print(f"Trust store: {trust_store}")
    print(f"Local HTTPS CA: {ca}")
    print(f"Local HTTPS certificate: {certificate}")
    print(f"Local HTTPS key: {key}")
    print("")
    print("Start the test server:")
    print(
        f"  python3 scripts/example-store-server.py --root {output} --port 8787 "
        f"--tls-cert {certificate} --tls-key {key}"
    )
    print("")
    print("Website source:")
    print("  https://127.0.0.1:8787/site/store.json")
    print("GitHub source:")
    print("  example/apps")
    print("  (start the platform with RUMAHL_GITHUB_RAW_BASE=https://127.0.0.1:8787/github)")


if __name__ == "__main__":
    main()
