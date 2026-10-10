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
- The platform service gives every installed app its own directory under the
  apps root (`RUMAHL_APPS_ROOT`, default `/apps`; dev/test override) and creates
  it owner-only (`0700`) on install. `GET /api/v1/shell/apps/{id}/data` exposes a
  read-only listing and file view of that directory.
- Wiring `/home` to the data disk and provisioning the per-account
  subdirectories remain follow-up work.

## App data access

An app's data directory is addressed as `<appsRoot>/<appId>`. Reading it is
owner-scoped and requires advanced mode or above (`can_browse_system_files`);
the path walk rejects `..`, absolute paths, symlinks and escapes with `openat`
plus `O_NOFOLLOW`, and caps listings and file previews. Apps receive their own
directory on install; how a running app writes there (container volume or
runtime sandbox) is a separate milestone.
