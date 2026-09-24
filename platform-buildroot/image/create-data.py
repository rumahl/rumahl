#!/usr/bin/env python3
"""Create a NEW persistent QEMU /var disk from explicitly provisioned seed data."""
import argparse
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--seed", required=True, type=Path)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()
seed = args.seed.resolve(strict=True)
for required in ["lib/rumahl/accounts.sqlite", "lib/rumahl-tls/fullchain.pem", "lib/rumahl-tls/key.pem"]:
    if not (seed / required).is_file():
        parser.error(f"missing provisioned {required}")
# Exclusive creation prevents accidental formatting of an existing disk/device.
fd = os.open(args.output, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
with os.fdopen(fd, "wb") as disk:
    disk.truncate(1024 * 1024 * 1024)
subprocess.run(["mke2fs", "-q", "-t", "ext4", "-L", "rumahl-data", "-E", "root_owner=0:0", "-d", str(seed), str(args.output)], check=True)
print("Created persistent data image", args.output)
