# Buildroot migration from the prototype

Reference reviewed: [rumahl-prototype, commit ce9e5eb](https://github.com/kaimdt/rumahl-prototype/tree/ce9e5eb4333012d50a89c25f0ac4a8276e493eb2/rumahl-os).
This is an adaptation for the second-generation platform, not a copy of the old
runtime or a claim that the prototype's image tests cover this implementation.

| Prototype observation | Direction in this repository |
| --- | --- |
| An external Buildroot tree, systemd services and QEMU support already exist. | Preserve those mechanisms with one small x86-64 acceptance target first. |
| `configs/rumahl_defconfig` pins Buildroot-era kernel 6.6.15, specifies a fixed root password, and references board files absent from this commit's tracked tree. | Pin a newer source archive and checksum, use upstream QEMU board files, disable root login, and check every new board file into this repository. |
| The image builder handles many formats, native-service discovery, environment recovery and fallback rootfs payloads in one large script. | Separate target compilation, frontend bundling, artifact staging, image creation and acceptance. Missing platform artifacts fail the build. |
| The full-image path describes root slots plus separate data, while an explicit fallback can be non-bootable. | Keep durable data separate from the system. Initially boot the explicit kernel + rootfs pair in QEMU; never label a rootfs-only payload as a bootable installer. |
| The development VM is Debian-based and runs the old service set. | Use host process tests for fast feedback and a distinct Buildroot VM acceptance step. Report which environment was actually tested. |
| Many product services, container tooling and developer components are included. | Make the first login milestone depend only on platform, SSR and HTTPS. Add the runtime supervisor and other image capabilities when their flows are integrated. |

The new implementation uses SQLite-backed accounts, atomic browser sessions,
host-only cookies, CSRF checks and isolated service sockets. The old account,
service registry and networking conventions are not imported into the domain
core. The UI stays usable without a container engine.

Two boundaries remain explicit: the current recovery route is informational,
and read-only rootfs plus a persistent data disk is not an A/B update system.
RAUC, signed boot/update policy, rollback, TPM sealing and device-specific
provisioning remain separate milestones with failure/power-loss tests.

See [the new image build instructions](../../platform-buildroot/image/README.md)
and [the executable HTTPS acceptance test](../../tests/e2e/shell/check.py).
The user's two frontend architecture drafts are intentionally not overwritten.
