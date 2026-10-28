# App store and sources

Status: native Store shell app, source management and signed-package install
path. The store is deliberately source-agnostic.

## Native app

`StoreApp` is a trusted first-party React app in the shell, not a sandboxed
installed app. It is available as `/app/store` and can be opened from the app
grid, the app catalogue and Settings → Store sources. It never trusts source
metadata as executable content.

## Storefront

The Store is a first-party app with `customTopbar: true` and a preferred initial
window size of 1200×760. The old 1040×720 default is treated as superseded for
saved Store positions; explicit user resizes remain untouched. Its native
titlebar hosts the Store brand, global search and refresh; window controls,
dragging and double-click-to-maximize remain owned by the shell. In fullscreen, where the titlebar is hidden, the search and refresh
actions move into a compact in-content toolbar.

The app follows the native Settings window metrics rather than the standalone
prototype scale: a 238px sidenav, 39px navigation rows, 35px/45px content
padding, a 38–48px editorial page title and 12px body copy. Search fields,
filter chips and buttons use the Settings control scale (36px height, 12px
label, 14px inline padding); compact app-card actions also use 36px so no store
control is smaller than its Settings counterpart. The page uses the native
window glass with a subtle panel tint and no custom gradient. The sidenav and content
are two independent panes: the sidenav stays fixed while the content column owns
its scroll position, matching Settings and other two-pane OS apps. Editorial
content is capped at 1320px and centred when the window is wider, so maximized
windows do not stretch the cover, cards or chart into unusable sizes. On wide
windows the card/row type scale also steps up (The Edit lead 17px/12.5px,
compact 13.5px/11px, chart 12px/10.5px, registry 12.5px/10.5px) instead of
leaving 11px copy inside 400px cards; 10.5px is the metadata floor in these
compact rows.

Discover is composed as a native marketplace, not a catalog list:

- A Microsoft-Store-style banner mosaic presents up to six apps. A large
  rotating banner on the left keeps the app screenshot crisp (no blur) behind
  the app icon, editorial copy, rating/download metadata, install/open and
  details actions, with count/dots/previous/next controls below it. A light
  left/bottom scrim keeps the editorial copy readable without hiding the art.
  The right column contains three full-bleed promo banners: one wide lead and
  two smaller cards, each linking to its app and using the same crisp treatment.
  Below 1050px the mosaic stacks; below 760px the lead promo spans the row and
  the two smaller promos sit beneath it. Sources without a featured app are
  represented in the remaining sections rather than being forced into a
  featured slot. Installed state uses a compact pill badge rather than plain
  uppercase text. Category, registry and app cards press down with the native
  card scale, matching buttons.
- Motion stays restrained: staggered section/card entrances, hover lift and
  side-banner image zoom. The slow banner-art drift is **not** tied to the
  general animation switch alone; it follows the Appearance setting
  "Animated backgrounds" (`wallpaperMotion`, also used by the wallpaper). With
  that setting off the banner is still. The shell's animation setting
  (`data-animations="off"`) or the OS reduced-motion preference disables all
  motion, including the rail smooth scrolling.
- The Store is the reference consumer of the UI package's official
  `AppNavLayout` (`@rumahl/ui/components`): optional navbar slot, desktop
  `Sidenav` rail, content scroller, and the standard `MobileIslandNav` supplied
  through `mobileNav`. Below 760px the rail and titlebar search collapse into the
  floating island; it carries Discover, All apps, Categories, My apps, Sources
  and Search, with the active destination as a sliding accent pill. Tapping a
  destination animates the pill into place; dragging it across destinations
  previews the target and commits on release. Arrow/Home/End keys move the
  selection. The same animated shared pill is used by RumahlButtonGroup,
  RumahlTabs and the converted Settings segmented groups. Content gets bottom clearance so the last row can scroll clear
  of it, and the island persists on detail pages. Island material and metrics
  are `--island-*` recipe variables, with a light/square clean-theme variant.
  The contract is documented in Storybook under `Layout/App/Navbar + sidenav`
  and `Components/Mobile island`.
- Search now has a dedicated view: the island's Search entry opens it, focuses
  the mobile search field and shows result rows. The desktop topbar search feeds
  the same view. Search stays active while opening an app or developer page, so
  Back returns to the same query and results.
- The Edit is an editorial selection: one wide media-led lead followed by two
  compact Media-style rows with icon, title, publisher and install/open action.
  The lead uses the same shell-less screenshot-card composition as All apps;
  compact rows use the lighter list-card rhythm. Featured apps are preferred,
  followed by category-diverse picks.
- Top charts is a compact download-ranked set of white list cards with rank,
  icon, title, publisher/rating and Get/Open, followed by a source trust card
  and the "software that stays yours" principle. The chart may repeat an app
  from the cover or The Edit because it represents a different piece of
  information.
- Collections are horizontally browsable gradient cards with the lead app's
  screenshot as a tilted deck, title, category app count and native scroll
  control. The category preview rails reuse the same media-led app card as All
  apps.
- More from your sources is a two-column registry index for additional apps that
  are not already surfaced in the cover, edit or chart.
- The source/trust footer closes the page. Empty sections are omitted; missing
  featured flags fall back to the download-ranked catalogue.

Recommendation data stays source-owned: featured flags, ratings, downloads,
categories, icons and screenshots are optional metadata. The Store never
invents a weekly ranking, verified-publisher badge, account requirement or
network permission. A package with no declared permissions is shown as
"None declared", not as a security claim.

The native sidenav contains Discover, All apps and My apps, followed by every
enabled source with its live status and app count, and Manage sources. All apps
uses a screenshot-led catalogue grid with contextual category chips: a wide
banner, icon/title/install row, two-line summary and rating/download/category
metadata, with no outer card shell. The catalogue is two columns at the native
window width and three columns when the content pane is wide. Search switches
to a denser result list. My apps is an in-Store library of installed applications
with open actions and keeps the compact catalogue grid. Narrow OS windows
replace the sidenav and titlebar search with the bottom island, move the cover
pager to the top-right, stack the editorial layout, and switch The Edit to a
single column below 560px; its compact rows are full-width at every size. The detail page stacks its hero,
preview, support facts and source panel without hiding the install action. The
content column remains the scroll owner at every width.

Application detail is a page inside the Store rather than a metadata modal. A
compact back bar returns to the previous scroll position. The banner uses the
first screenshot as a blurred backdrop; the app icon sits at the far left, the
title/developer/category in the middle, and the install/open action at the right.
Summary, rating, downloads and version sit in a separate glance strip below the
banner rather than crowding the header. The developer name is a button that
opens a developer page: a matching blurred banner, the publisher name, app/source
counts and every matching app from the enabled sources. A developer page opened
from an app returns to that app; apps opened from the developer page return there.
A "What this app really needs" panel summarizes runtime, declared
permissions and source provenance in plain language, links through to the
permission and package detail sections, and explicitly notes when internet,
offline and account requirements are not declared by the source. These are
descriptive facts, not badges that imply more than the metadata says. Below it are a large preview with a "local by design" note, the
product summary, source-declared permissions, a "More from this source" list and
a sticky install/source-package panel. The signed-package verification note
remains visible beside the install action.

Recommendation titles, summaries, icons, screenshots, categories, ratings,
downloads, featured flags and permissions come from optional source metadata.
Missing or failed media falls back to the theme's monogram gradient. Source
errors remain visible, every configured source stays reachable in the sidenav,
and source settings remain available even when no index loads. Installation stays
on the signed-package path and is disabled when the install feature is unavailable.

## Sources

A source is an HTTPS URL to a JSON index. Sources are device-local settings and
can be added, disabled or removed in **Settings → Store sources**. The shell
does not fetch arbitrary source origins directly: its CSP is
`connect-src 'self'`, so source and package documents are retrieved through the
authenticated same-origin bridge:

```text
GET /api/v1/shell/store/fetch?url=https%3A%2F%2Fstore.example%2Findex.json
```

The bridge accepts HTTPS only, rejects embedded credentials and fragments, does
not follow redirects, applies a 15 second timeout and limits a document to
32 MiB. Source authors should serve JSON.

### Index format

```json
{
  "storeVersion": 1,
  "name": "Example Store",
  "apps": [
    {
      "id": "com.example.notes",
      "title": "Notes",
      "version": "1.0.0",
      "summary": "A local-first notes app.",
      "publisher": "Example",
      "packageUrl": "https://store.example/packages/notes.package.json",
      "category": "Productivity",
      "rating": 4.8,
      "downloads": 18400,
      "featured": true,
      "permissions": ["Read personal files", "Show notifications"],
      "iconUrl": "https://store.example/media/notes-icon.png",
      "screenshots": ["https://store.example/media/notes-screen.png"]
    }
  ]
}
```

`packageUrl` may be relative to the index URL. It must resolve to HTTPS. App IDs
must be valid rumahl app IDs. The shell only displays source metadata; it never
executes source JavaScript or styles.

Optional metadata drives the storefront's editorial layout:

- `category` or `categories` (up to six strings) fill the category filter and
  the category tags on cards and detail pages.
- `rating` is a number from 0 to 5; `downloads` is a non-negative integer.
- `featured: true` is preferred for the cover carousel and The Edit. The chart
  and registry remain download/rating-driven so they do not become a second
  featured shelf.
- `permissions` is an optional list of human-readable permissions declared by
  the source. The detail page shows it as source metadata; the signed package
  manifest remains authoritative.
- `iconUrl` and `screenshots` must use the same origin as the index. They are
  fetched through `GET /api/v1/shell/store/image?url=…`, because the shell CSP
  only permits same-origin images. Only PNG, JPEG, WebP, GIF and AVIF are
  proxied, up to 8 MiB each. Invalid optional metadata is ignored; required
  fields fail the index.

The image bridge uses the same authentication, certificate trust and redirect
policy as the JSON bridge. Cross-origin artwork is deliberately rejected so a
source cannot load images from an untrusted host behind the user's back.

### Certificate trust

The fetch bridge uses the normal web PKI. If a source presents an untrusted
certificate, the bridge performs a short TLS probe and answers with `428
Precondition Required` and a JSON body containing the host and the SHA-256
fingerprint of the leaf certificate. The Store explains the risk and asks the
user to trust it or cancel. A confirmed certificate is pinned per OS account and
host in `$RUMAHL_STATE_DIR/app-store-trust.json`; package URLs on the same host
use the same decision. If the certificate changes, the next fetch asks again.
Only the leaf fingerprint is pinned; TLS handshake signatures are still
verified. System-trusted certificates never trigger the prompt.

### Package document

A source package document is a JSON envelope carrying the same files as a
signed rumahl package directory:

```json
{
  "packageVersion": 1,
  "files": [
    { "path": "package.json", "content": "<base64>" },
    { "path": "package.sig.json", "content": "<base64>" },
    { "path": "frontend/index.html", "content": "<base64>" }
  ]
}
```

The shell validates the envelope, then sends the files through the existing
authenticated `POST /api/v1/shell/apps/import` endpoint. The Rust importer
verifies the detached Ed25519 signature and every payload digest against the
installed trust store, publishes the assets and executes the normal install
journal. A store cannot bypass package signatures or install permissions.

## Security boundaries

- Store sources are untrusted catalog/package providers.
- The Store app never runs source code; it renders bounded text metadata, uses
  same-origin-proxied media and falls back to generated monogram tiles when media
  is missing or fails.
- All package signatures and payloads are verified server-side before install.
- Store installs are allowed in Guided mode because they are explicit first-party
  actions from an index the user configured and every package is signature
  verified. Manual package-folder imports remain restricted to Advanced and
  Developer modes.
- The fetch bridge is authenticated and same-origin; it does not forward shell
  cookies to the source.
- Adding a source is an explicit user action. A source URL can still target a
  host reachable by the device, so only add sources you trust.

## Local example store

`scripts/dev-example-store.sh` builds and signs twenty example apps, writes the
index/package documents and starts a local HTTPS server. If Playwright is
available it also captures an icon and screenshot for every app so the media
bridge and editorial layout can be tested; otherwise the store falls back to
generated monogram artwork. Its certificate is self-signed, so the Store shows
the certificate prompt once. The signing key is added to the development trust
store used by the app importer.

## Follow-up work

- server-side source allowlisting / reputation and private-network policy;
- source signing or transparency metadata for the index itself;
- update/uninstall flows and a source-aware app catalog;
- cached/signed source metadata for offline browsing.

## Product gallery and richer metadata

Discover also includes up to three category preview rails, each with up to six
apps and navigation into the full category. These are derived from source data.
The product header displays the actual publisher directly below the app name.
Apps by the same publisher (within the same source) are grouped in the detail
page; other source apps remain a separate section.

Indexes may optionally provide plain-text `description` (up to 12,000 characters)
and `releaseNotes` (up to 6,000 characters). The detail page falls back to summary
and omits release notes when absent. Text is rendered as text, never HTML.

Screenshot previews open a Store-scoped lightbox rather than an OS-level dialog.
It portals into the owning shell window and covers that window only, including its
titlebar, while the rest of the desktop remains untouched. The dark stage keeps
the screenshot's aspect ratio, with previous/next controls, arrow-key navigation,
thumbnails, fit/native-size zoom, a failed-image state and Escape to close. The
window's content and titlebar become inert, Tab cycles inside the viewer, and
focus returns to the opening control. All image URLs still pass through the
existing same-origin metadata validation and store image proxy.

## Store navigation

The prototype at https://store.rumahl.com/ informed the separation of browsing,
installed apps and package origins, not the visual styling. The current shell
keeps its own source-based model: Discover, All apps, Categories, My apps, and
Source overview. The overview explains enabled catalogs versus local installation
and lists enabled, failed and disabled sources. The prototype's separate registry,
themes and update services are not implied by this catalog interface.

Category overview includes every category present in enabled catalogs; Discover
limits its featured category rail separately. Catalog searches retain the current
source/category scope, with removable filter chips. Library search stays within
installed apps, including when no source is configured. Catalog-backed library
entries provide details as well as Open.

Product pages retain their parent selection, query and return scroll position.
Their back action names that context. Section jumps use per-instance IDs so two
Store windows do not share anchor destinations. A source action opens the actual
catalog that supplies the selected package.
