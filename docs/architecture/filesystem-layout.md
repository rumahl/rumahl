# Filesystem layout

rumahl OS uses real host paths and defines a small, protected top-level layout
instead of an abstract path namespace.

| Path | Purpose | Protection |
| --- | --- | --- |
| `/rumahl` | Protected system tree: rumahl system services and the admin home. | Mode `0700`, read-only at runtime. |
| `/rumahl/root` | Admin home; replaces the classic `/root`. | Mode `0700`. |
| `/apps/<appId>` | Installed application storage, one directory per app id. | Separated per app. |
| `/home/<user>` | User home with the standard subdirectories. | Owner-only. |

`/root` deliberately does not exist; `/rumahl/root` takes its place.

Every account gets these standard subdirectories under its home:

`Desktop`, `Documents`, `Downloads`, `Music`, `Pictures`, `Public`, `Templates`,
`Videos`.

The canonical roots are defined once in Rust (`FilesystemLayout` in
`rumahl-platform-buildroot`), so services and, later, the shell and apps agree on
the same names.

## Read-only system, writable data

The system disk is read-only, so `/rumahl` (including `/rumahl/root`) cannot
change while the system is running. Persistent, writable state lives on the
separate data disk mounted at `/var`. `/home` and `/apps` are stable global
names for writable storage and are backed by the data disk.

## Customization

The roots are fixed for the first milestone. Buildroot will expose them as
configuration so an image can relocate `/rumahl`, `/apps` and `/home`.

## Status

- The image (`platform-buildroot/image/board/qemu/post-build.sh`) removes `/root`
  and creates `/rumahl` and `/rumahl/root` with mode `0700`.
- Wiring `/home` and `/apps` to the data disk, provisioning the per-account
  subdirectories, and exposing the layout to the shell, terminal and apps
  through the platform API are follow-up work.
