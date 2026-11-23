# Real-hardware acceptance (x86-64 UEFI)

This checklist validates the disk image on a physical mini-PC before further
image work (A/B updates, Secure Boot, PCR policy). Record only public evidence;
never copy passwords, session cookies or TPM secrets into a report.

## Artifacts

- Image: `images/rumahl-disk.img` from the `rumahl_x86_64_uefi_defconfig` build.
- Record: image SHA-256, source commit, shell build ID, device model and UEFI
  firmware version.
- The image is built with a **development package trust store** unless a
  release trust store was supplied; do not treat it as a release artifact.

## Prerequisites

- x86-64 mini-PC with UEFI boot. Disable Secure Boot for this image; GRUB and
  the kernel are not signed yet.
- Wired LAN with DHCP. TPM 2.0 is optional (the secret store stays disabled
  without it).
- HDMI monitor and USB keyboard, or a serial console. The first-boot wizard
  uses `/dev/console`.

## Flash

Flash a USB SSD/stick or the internal disk. This erases the whole device:

```sh
sudo platform-buildroot/image/flash.sh \
  <build>/images/rumahl-disk.img /dev/sdX --grow
```

`--grow` expands the 8 GiB data partition to fill the device. Boot the mini-PC
from the device in UEFI mode.

## First boot

The console wizard asks for admin user, display name, hostname and password
(minimum 15 characters). It then:

- creates the device CA (`/var/lib/rumahl-ca`) and an initial TLS certificate;
- provisions a TPM-sealed secret-store key at `0x81000001` when a TPM exists;
- writes the first account and the persistent directories;
- starts Avahi (`rumahl.local`) and iwd (if wireless hardware exists).

The console prints the reachable addresses. The first-boot log is readable at
`/var/log/rumahl-first-boot.log` on the console.

## Host-side checks

From a laptop on the same network, copy the device CA from the console
(`cat /var/lib/rumahl-ca/ca.pem` as the admin) and run:

```sh
python3 tests/e2e/hardware/check.py --origin https://<device-ip> --ca ca.pem \
  --username <admin>
```

For a first smoke test `--insecure` skips TLS verification; the script prints
the certificate SANs so the IP/host coverage can still be checked.

## Checklist

- [ ] UEFI boots the image from the target device.
- [ ] First-boot setup completes; the log shows account, CA and TPM status.
- [ ] TPM sealing reports `provisioned at 0x81000001` (if TPM present).
- [ ] `https://<device-ip>/` shows the sign-in page; certificate includes the
      device IP and the configured host names.
- [ ] Admin login succeeds; shell and Settings → This device show the
      addresses.
- [ ] `/recovery` is reachable and the diagnostics render.
- [ ] `rumahl.local` resolves from an mDNS-capable client (or use the IP).
- [ ] Reboot: session and device data persist.
- [ ] Power loss: device boots again and data persists.
- [ ] Optional: install a signed app from the configured trust store and
      launch it; app data survives a reboot.
- [ ] Optional Wi-Fi: `iwctl station <device> connect <ssid>` or the
      first-boot `FIRST_BOOT_WIFI_*` profile.

## Evidence to record

- Image SHA-256, commit, shell build ID.
- Device model, UEFI firmware version, TPM present/absent.
- Addresses shown on the console and by the check script.
- PASS/FAIL for every checklist item, plus the first-boot log if a step fails.

## Known limitations for this acceptance

- Secure Boot must stay disabled (unsigned boot chain).
- No installer, A/B slots or rollback; the whole disk is flashed.
- TPM sealing uses the owner hierarchy without a PCR/measured-boot policy.
- Avahi publishes the fixed name `rumahl.local`; the hostname prompt is stored
  for the UI but does not rewrite the read-only Avahi configuration.
- Wi-Fi is managed by iwd but has not been exercised on physical hardware here.
