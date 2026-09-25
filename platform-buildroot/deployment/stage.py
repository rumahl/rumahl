#!/usr/bin/env python3
"""Stage target-matching binaries and a built frontend into a NEW image overlay."""
import argparse
import json
from pathlib import Path
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
frontend = repo / "frontend/packages"
build = json.loads((frontend / "shell/dist/build-id.json").read_text())
renderer = (frontend / "ssr/dist/render.js").read_text()
if build["shellBuildId"] not in renderer:
    parser.error("renderer and client build do not match; rebuild both")
if not args.binary.is_file():
    parser.error("target platform binary is missing")
args.output.mkdir(parents=True, exist_ok=False)
root = args.output

def copy(source, target):
    target = root / target
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)

copy(args.binary, "usr/bin/rumahl-platform-service")
(root / "usr/bin/rumahl-platform-service").chmod(0o755)
for package in ["shell", "ssr"]:
    shutil.copytree(frontend / package / "dist", root / f"usr/share/rumahl/frontend/packages/{package}/dist", ignore=shutil.ignore_patterns("*.map"))
copy(frontend / "ssr/server.js", "usr/share/rumahl/frontend/packages/ssr/server.js")
(root / "usr/share/rumahl/frontend/packages/ssr/package.json").write_text('{"type":"module"}\n')
for name in ["rumahl-platform", "rumahl-shell"]:
    copy(repo / f"platform-buildroot/systemd/{name}.service", f"usr/lib/systemd/system/{name}.service")
    wants = root / "etc/systemd/system/multi-user.target.wants"
    wants.mkdir(parents=True, exist_ok=True)
    (wants / f"{name}.service").symlink_to(f"/usr/lib/systemd/system/{name}.service")
copy(repo / "platform-buildroot/deployment/platform.env.example", "etc/rumahl/platform.env")
copy(repo / "platform-buildroot/deployment/rumahl.sysusers", "usr/lib/sysusers.d/rumahl.conf")
copy(repo / "platform-buildroot/nginx/rumahl.conf", "etc/nginx/rumahl.conf")
copy(repo / "platform-buildroot/nginx/rumahl-proxy.inc", "etc/nginx/rumahl-proxy.inc")
dropin = root / "etc/systemd/system/nginx.service.d"
dropin.mkdir(parents=True, exist_ok=True)
(dropin / "rumahl.conf").write_text("[Unit]\nWants=rumahl-platform.service\nAfter=rumahl-platform.service\nRequiresMountsFor=/var/lib/rumahl-tls\n")
copy(repo / "LICENSE", "usr/share/licenses/rumahl/LICENSE")
print("Staged", build["shellBuildId"], "into", root)
