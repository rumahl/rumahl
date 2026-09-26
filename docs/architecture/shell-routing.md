# Shell routing and presentation

This document describes the implemented routing foundation and authorized static
web-app host. Package import/publication and container web proxies remain separate
work; an installation record alone is not enough to launch an app.

## One address, two presentations

The URL identifies content; desktop/launcher identifies presentation. A request
for `/app/test/documents/42` keeps that URL. The local gateway authenticates the
browser and renders the shell for the requested path. There is no redirect to
`/` and no second application-specific HTML document on the trusted shell host.

| URL | Meaning |
| --- | --- |
| `/` | Desktop dashboard; app grid in launcher mode |
| `/apps` | First-party apps and authorized installed-app catalog |
| `/app/:appId/*` | App identifier and an app-owned nested route |
| `/settings/*` | Settings module and its nested pages |
| `/settings/display` | Shell presentation controls |
| `/activity` | Activity view (currently a placeholder) |
| `/streams/:sessionId` | Existing authorized graphical stream host |

Desktop/launcher is a persisted presentation preference, never a route parameter.
The top-bar selector changes this browser profile's override. `/settings/display`
lets the user choose account-wide or device scope and remove a device override.
The mode does not change the current path, query or history. Old `mode` query
parameters are removed with history replacement and do not override preferences.
See [shell preferences](shell-preferences.md) for persistence and synchronization.

A reload reconstructs the active route, not the entire previous desktop session.
Multiple windows remain per-tab state; switching mode retains their descriptors
but remounts active view content. SSR and initial hydration use the same default
presentation; authenticated preferences are applied after hydration.

Navigation IDs accept up to 255 ASCII characters (letters, digits, `.`, `_`, `-`);
installed-app IDs additionally follow the authoritative Rust AppId schema. Use
`appPath(id, segments)` to encode individual subpath segments. IDs are not
filenames, module imports, URLs, or authorization decisions. Unknown apps and
unknown nested pages get explicit unavailable/not-found views. There is no
catch-all mapping of arbitrary root names to app IDs.

## Responsibilities and extension points

- `App.tsx`: live snapshot/session boundary, locale provider, router, shell.
- `routing/ShellRouter.tsx`: React Router browser, SSR and test adapters.
- `routing/routes.tsx`: first-party shell route table with navigation/title and
  page/window metadata; the same table matches and renders every surface.
- `routing/ShellLink.tsx` and `paths.ts`: links and canonical app URL construction.
- `pages/`: route views, including nested settings and generic app/stream hosts.
- `apps/registry.tsx`: trusted first-party app declarations and nested routes.
- `shell/ShellLayout.tsx`: presentation choice and route/window coordination.
- `shell/DesktopWindows.tsx`: protected controls, focus, minimization and close.
- `shell/Topbar.tsx`, `CommandPalette.tsx`: independent shell UI.
- `routing/RouteBoundary.tsx`: client render failure isolation per route/window.

A new first-party app registers an ID, translation key and React Router route
objects in the registry. For example, a notes module could contribute an index
view and `documents/:documentId`. Its views use router params/search params;
no app-specific switch is added to `App.tsx`. A new first-party system page is
added to the shell route table and implemented in its own module. The existing
module boundary supports nested route/layout objects; heavy views can later
introduce loading boundaries when needed. There is no custom URL matcher or
replacement for React Router's History API handling.

Third-party app manifests **cannot** register React components in this table.
The Rust-provided, session-filtered catalog/launch API is consumed by
`apps/AppCatalog.tsx` and `apps/InstalledAppHost.tsx`. The host uses an authorized
descriptor and a sandboxed frame on an installation-specific HTTPS host, never
interpolates an app ID into an arbitrary URL, and revalidates access while open.
See [Installed app hosting](app-hosting.md) for authorization, asset publication
and deployment requirements.

## Server, login and hydration

Rust sends the bounded `requestPath` alongside the snapshot, nonce and approved
frame origins on the private renderer protocol. No browser credentials enter
SSR. StaticRouter renders that path; BrowserRouter hydrates the same browser
location. Query strings are preserved; fragments remain browser-local.
Production and development use the same renderer and routing source.

Unauthenticated deep links redirect to `/login?next=...`. The form preserves a
validated local shell target, including after invalid credentials. POST login
validates it again. External/protocol-relative targets, backslashes, controls,
and reserved namespaces are rejected. API, static assets, auth/OIDC and recovery
keep their own handlers; their missing endpoints do not fall back to shell HTML.
Unknown shell pages display a not-found view inside an authenticated shell.

A route error does not grant access or disable Rust checks. Stream session URLs
still obtain grants through the existing backend. Independent `/recovery`
remains outside React routing and remains available when SSR fails.

## Verification and remaining work

Unit tests cover registration of a fixture app without editing the router,
nested parameters/query, window lifecycle, both modes, unavailable apps and safe
paths. SSR tests cover direct paths, mode coherence and invalid renderer input.
Rust tests cover the renderer protocol and safe login return targets. Real HTTPS
and Chromium checks cover login return, reload, backward/forward navigation,
mode switching, hydration and Fast Refresh. The production HTTPS test stages
the renderer without node_modules, as in the OS image.

Next steps are production package import/publication, a reviewed app action
bridge and richer desktop/window interactions. The
prototype's visual desktop, file operations and launcher layout are not copied
wholesale. Settings/activity retain their existing product placeholders.
