# rumahl shell renderer

This is a first-party renderer, not the public web gateway. Build the client and renderer together with `pnpm build` from `frontend/`. The client build emits `dist/build-id.json`; the renderer refuses to start if its build ID differs. Run `pnpm --filter @rumahl/ssr test:smoke` after building to exercise the Unix-socket HTTP boundary.

Set `RUMAHL_SSR_SOCKET` to an absolute path in a private, pre-created directory. The default socket is owner-only (`0600`). If the Rust gateway and renderer run as different users, provision a dedicated shared group and a non-world-accessible directory with group traversal, then set `RUMAHL_SSR_SOCKET_ACCESS=group`; the socket becomes `0660` only when its group matches the directory group. Do not expose this socket to apps or the public network.

The gateway must authenticate the browser session and authorize visibility **before** constructing the snapshot. It sends `{ "snapshot": ..., "nonce": ..., "frameOrigins": [...] }` to `POST /render`, without browser cookies or identity headers. `frameOrigins` is an optional allowlist of exact HTTPS origins resolved from authorized app contributions; it is used only for CSP. The gateway applies response caching and security headers at the public edge. A Unix peer identity proves only which service connected; it is not a user identity. The renderer has no database, container engine, or secret-store access.

The public HTTP adapter and Buildroot service deployment are not implemented here yet. In particular, the renderer is not a substitute for the independent `/recovery` route.
