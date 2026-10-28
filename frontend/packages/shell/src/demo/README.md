# Demo

The demo uses the production shell runtime, API clients, contract validation,
React components and state management. Only the HTTP and event transports are
replaced by `DemoBackend`, so the code path is the same one the real gateway
uses.

## What the demo contains

- sample apps (Notes, Weather, Photos, Music, Home) served under `/apps/*`
- interactive desktop widgets (Home, Weather, Energy)
- a seeded file store (Desktop, Documents, Downloads, Pictures, Music) with
  editable files on the desktop
- persistent preferences, workspace, OS mode and files in `localStorage`
- theme switching, launcher pages, app windows, read-only host roots and
  per-app settings/data views

The widget and sample-app host is a standalone static page
(`demo-widget.html`). By default it is served by the same demo deployment under
`/apps/*` and `/widgets/*`; the frames still run in an opaque sandbox. For a
stronger origin boundary the same page can be deployed separately and selected
with `RUMAHL_DEMO_WIDGET_ORIGIN`. Production app frames stay bound to their
per-installation origins; the demo frame origin is accepted only when the demo
entry passes `demo: { frameOrigin }` to `startShell`.

## Local development

```sh
pnpm --dir frontend dev:demo
```

This starts the shell on the Vite dev server and the widget/app host on
`http://127.0.0.1:5174`. Both are blocked outside demo mode.

## Static builds

```sh
pnpm --dir frontend/packages/shell build:demo
```

The demo build produces `dist-demo/` with:

- `index.html` — the shell
- `demo-widget.html` — the widget/app host
- `/apps/<app-id>/` and `/widgets/<slug>/` routed to the host by the supplied
  Vercel/Netlify config

All app and widget frames therefore live on the same origin as the demo. They
are still embedded in an opaque `sandbox="allow-scripts"` iframe, so a demo app
cannot read the shell DOM or its browser storage. This avoids one domain (and
certificate) per app.

`pnpm --dir frontend/packages/shell build:demo-widget` is optional. It builds
`dist-demo-widget/` for deployments that prefer a dedicated origin
(`demo-widget.rumahl.com`). Set `RUMAHL_DEMO_WIDGET_ORIGIN` at build time to use
it; otherwise the demo serves the host on its own origin. Local development uses
`http://127.0.0.1:5174` automatically.

### Deployment

Create a static project with the root directory `frontend/packages/shell` and
deploy `dist-demo` to `demo.rumahl.com`:

- Vercel: `vercel deploy --local-config vercel.demo.json --prod`
- Netlify: `netlify deploy --config netlify.demo.toml --dir dist-demo --prod`

The configs add the `/apps/*` and `/widgets/*` rewrites and a CSP that keeps
the frame host embeddable by the demo only. Add the DNS record for
`demo.rumahl.com` through the hosting provider.

For the optional dedicated widget origin, deploy `dist-demo-widget` to
`demo-widget.rumahl.com` with `vercel.demo-widget.json` or
`netlify.demo-widget.toml`, and build the demo with
`RUMAHL_DEMO_WIDGET_ORIGIN=https://demo-widget.rumahl.com`.

The website only needs `NEXT_PUBLIC_RUMAHL_DEMO_URL=https://demo.rumahl.com`.

Builds are best-effort regarding upstream assets: `pnpm assets` may skip the
wallpaper download when offline, but a deployed demo should run it once with
network access.

## Path-based app hosting in production

The demo uses one origin for the shell and the frame host. The production
equivalent is a single app gateway instead of a wildcard domain per app:

```
https://apps.rumahl.com/<installationId>/<lease>/...
```

The gateway validates the short-lived lease, forwards only to the matching app
container, strips shell cookies and sets its own frame/CSP headers. Keep the
shell on a separate origin. Do not serve third-party app documents under the
shell origin in production: same-origin paths are not an isolation boundary by
themselves, even though the sandboxed iframe still uses an opaque origin.
Prefer a dedicated app origin (not a subdomain of the shell) so host-only
`__Host-` cookies cannot leak, and avoid a shared `Domain=.rumahl.com` cookie.

## Adding a demo API route

When a production API operation is added:

1. Add it to the shared API client and shared contracts.
2. Call that client from the regular feature code.
3. Add the same HTTP route, state transition and revision event to
   `DemoBackend`.
4. Test the feature through the shared runtime rather than importing demo data
   into production.

Production modules must never import from `src/demo`. The production bundle
verification rejects known demo markers, and the default development server
blocks demo entry paths.
