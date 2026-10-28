# rumahl Store — design QA

Date: 2026-10-26

## Current target

The Store must feel like a native rumahl OS marketplace, not a website inside a
window. The editorial composition (cover, The Edit, Top five, categories,
registry) is retained from v5, but scale and surfaces now come from the native
Settings app instead of the standalone prototype.

The earlier v4 prototype was rejected because it was visually a web store even
when rendered inside the native chrome. The standalone v5 prototype was also too
small and too custom in its page background; it remains a composition reference,
not a source of chrome metrics.

## Implementation

- Real shell chrome: 1040×720 preferred window, native titlebar, menu bar, dock,
  wallpaper and window controls.
- Native rumahl controls: Button, IconButton, Chip, SidenavItem, RumahlInputGroup,
  RumahlModal, GroupContainer and GroupRow.
- Palette matches the Settings app: panels and controls use `--nd-row`,
  `--nd-row-hover`, `--nd-button`, `--nd-divider` and `--nd-muted`, with no
  bespoke glass sheen, rim or colour gradient. The page keeps the native window
  glass and theme accent/text roles; cover and category artwork stay intentionally
  dark, with category tints derived from the theme accent.
- Native Settings metrics: 238px sidenav, 39px navigation rows, 35px/45px
  content padding, 30px page title, 12px body copy; 20–24px section titles,
  42–46px serif product titles, 18px primary radii, 15px inner radii.
- The sidenav is fixed and the content column scrolls independently, matching
  native two-pane apps.
- Editorial content is capped at 1200px and centred on wide/maximized windows;
  the cover, The Edit, chart and category rail keep their v5 proportions instead
  of stretching edge to edge.
- Microcopy has an 11px floor. Wide windows step the editorial type up (The Edit
  15px/13px/12px, chart 14px/11.5px, registry 14px/12px) so 400px cards are not
  filled with 11px text.
- Cover carousel with native dots/arrows and a mobile top-right pager.
- The Edit is a 2×2 editorial grid with direct install/open actions.
- Top five is download-ranked and can repeat a cover app because it shows a
  different signal. Source trust and the local-software principle sit beside it.
- Categories are screenshot-backed cards for every category in the enabled
  sources. The registry lists additional apps without repeating cover/edit/chart
  apps.
- Detail is a page, not a modal: provenance, honest facts, preview, described
  permissions, More from this source and a sticky install/source panel.
- No invented weekly rankings, verified-publisher badges, account requirements
  or network permissions. Undeclared permissions say "None declared".

## Responsive behavior

- 1040px window: fixed 238px sidenav + scrolling editorial content, 2-column
  screenshot-led catalogue.
- Maximized window: 3-column catalogue, 1200px capped editorial column with v5
  proportions.
- 760px window: horizontal sidenav, single-column editorial stack, 2-column
  The Edit, stacked detail.
- 540px window: single-column The Edit and catalogue, compact sidebar, cover
  dots remain available, Manage sources remains reachable from the trust footer.
- Dark mode: theme surface roles are used throughout; cover/category art remains
  intentionally dark and text contrast is tuned with `--store-muted`.

## Evidence

Playwright rendered the real StoreApp and shell CSS against the live example
store at `https://127.0.0.1:8787/site/store.json`, with a second source at
`https://127.0.0.1:8788/site/store.json` for source diversity and duplicate-title
checks.

- `/home/dev/tmp/qa-os-final-top.png`: 1040px window with native OS metrics.
- `/home/dev/tmp/qa-os-final-edit.png`: editorial/content scroll.
- `/home/dev/tmp/qa-os-final-detail.png`: detail page at native type sizes.
- `/home/dev/tmp/qa-os-1920-max.png`, `/home/dev/tmp/qa-os-1920-max-edit.png`:
  maximized window with capped editorial width.
- `/home/dev/tmp/qa-os-1024.png`, `/home/dev/tmp/qa-os-700.png`: adaptive widths.
- `/home/dev/tmp/qa-os-settings.png`: native Settings reference for metrics.
- `/home/dev/tmp/palette-store-light.png` and `palette-settings-light.png`:
  light-mode palette comparison.
- `/home/dev/tmp/palette-store-dark.png` and `palette-settings-dark.png`:
  dark-mode palette comparison.
- `/home/dev/tmp/palette-rail-light.png` and `palette-rail-dark.png`: category
  rail palette after aligning tints to the theme accent.
- `/home/dev/tmp/qa5-dark-top.png`: dark theme surface reference.
- `/home/dev/tmp/top-left-light.png` and `top-left-dark.png`: merged
  navbar/sidenav surface and rounded content opening at 2×.
- `/home/dev/tmp/allapps-light.png` and `allapps-dark.png`: media-led app cards
  and flat category filter chips with the accent selected state.
- `/home/dev/tmp/gutter.png`: one-pixel seam audit for the titlebar/nav join.
- `/home/dev/tmp/discover-top.png`, `discover-mid.png` and `discover-bottom.png`:
  editorial header, media-led Edit lead, Media-style Edit rows, Top charts cards,
  Collections and registry cards.
- `/home/dev/tmp/discover-collections.png`: gradient Collection cards with the
  tilted screenshot deck.
- `/home/dev/tmp/categories-light.png`: the same Collection cards on the
  Categories overview.
- `/home/dev/tmp/discover-dark.png` and `discover-narrow.png`: the new card
  language in dark mode and at the narrow two-pane breakpoint.
- `/home/dev/tmp/allapps-light.png`, `allapps-dark.png`, `settings-store-controls.png`:
  Store controls matched to the Settings 36px control scale.
- `/home/dev/tmp/myapps-light.png`: compact library grid after the app-card
  change.
- `/home/dev/tmp/store-edit-light.png`, `store-edit-dark.png`,
  `store-detail-light.png` and `store-detail-dark.png`: asymmetric Edit layout
  and sans detail hierarchy after the latest pass.

## Verification

- Cover carousel arrow/dot navigation checked at default and 540px.
- Desktop, 1920px maximized, 1024px, 700px, detail and native Settings comparison
  inspected.
- Wide-window type scale and 11px metadata floor checked at 1920px maximized.
- Scroll restoration checked after opening and closing detail.
- Search, category filter, source filter, library and empty-result flows rendered.
- Light and dark palettes compared against Settings; store panels, controls,
  selected navigation and muted text now use the same flat Settings roles.
- Navbar and sidenav use one opaque `--store-nav-surface` (panel background
  with 4% theme accent, not a blue tint) with no divider under the nav; the full titlebar shares that surface, and the content's top-left
  radius opens from it. The content also carries a soft top/left inset shadow
  so the chrome reads as the higher layer. The inset shadow was reduced
  slightly. Verified in light and dark at 2×.
- Category filter chips are flat (transparent field, divider stroke, accent
  selected state) instead of raised glass pills. Verified in All apps light/dark.
- Store search is centered against the full window titlebar, with the brand and
  refresh control kept clear of the window controls.
- The featured cover splits into a surface copy panel and a full-height artwork
  panel; text never sits on the screenshot, and only a small bottom scrim is
  used over the image.
- The Edit uses one wide lead card plus two compact cards below it; the lead is
  larger but not a second hero, and compact cards keep their install action.
- Detail titles, preview notes and cover titles use the rumahl UI sans with
  weight/size/spacing instead of a serif face.
- Runs locally and Signed are promoted as hero trust pills. A full-width
  "What this app really needs" panel groups runtime, permissions and source in
  plain language, links to the permission/package detail sections, and marks
  internet/offline/account as undeclared by the source.
- Accent foregrounds (nav icons, links, quick actions, pills and store icon
glyphs) use a runtime `--rumahl-ui-color-accent-ink`, contrast-corrected
  against the panel; light accents darken instead of disappearing. Covered by a
  theme-selection test at WCAG AA.
- Press treatment now covers cards, rows, chart/registry links, category cards,
  cover details, quick actions and native buttons/rows; all use native scale
  and inset press treatment.
- `tsc --noEmit` passed after the Store search/featured/detail changes.
- ESLint passed for `StoreApp.tsx` and both locale files.
- Discover card polish: page and section type scale raised slightly; section
  rhythm is 28px; chart, registry and trust type scales aligned.
- App cards now follow the store.rumahl.com/store/apps card composition: wide
  screenshot banner, icon/title/install row, summary and rating/download/
  category metadata, without an outer card shell. All apps uses two columns at
  the native width and three when wide; Discover category rails reuse the same
  card at a 300px rail width; My apps keeps the compact three-column library.
- Discover polish: the page title is 38–48px; the Edit lead is a shell-less
  media card with two full-width Media-style rows; Top charts uses the same
  compact white row cards with rank badges; registry rows are individual cards;
  Collections are bright gradient cards with the lead screenshot as a tilted
  deck. The Categories overview uses the same Collection cards.
- Store controls now use Settings' control scale: 36px search, chips and
  buttons, 12px labels and action text. Compact card install/open buttons and
  Top charts rows use the same 36px height.
- The Store window now opens at 1200×760 instead of 1040×720; the old saved
  default is replaced, while genuine user resizes are preserved. At 1200px the
  catalogue reaches three columns and the editorial panes have more breathing
  room.
- Screenshot previews now use a Store-scoped lightbox. It covers only the Store
  window (titlebar included), dims the app behind it, and provides a counter,
  zoom, close, arrow navigation, thumbnails and Escape. The titlebar/content are
  inert while open and focus returns to the gallery opener.
- Evidence: `/home/dev/tmp/store-lightbox.png`, `store-lightbox-next.png`,
  `allapps-light.png`.
- Discover now opens with a Microsoft-Store-style banner mosaic: a large
  rotating hero on the left with a crisp screenshot backdrop (no blur), app
  icon, title, summary, metadata and install/open + details, plus one wide and
  two small full-bleed promo banners on the right. A light left/bottom scrim
  keeps copy legible without washing out the art. Count, dots and arrows sit
  below the hero. The mosaic stacks below 1050px and the lead promo spans the row
  below 760px. Evidence: `/home/dev/tmp/discover-top.png`, `store-banner-2.png`,
  `store-banner-3.png`, `store-banner-mobile.png`.
- Installed state is now a real pill badge in cards, rows and the detail hero
  (dot + tint + border) instead of plain uppercase metadata. Evidence:
  `/home/dev/tmp/allapps-light.png`, `store-installed-detail.png`.
- Discover motion is staggered page/card entrances, hover lift, side-banner
  image zoom and a slow banner-art drift; all animation/transition and smooth
  scrolling is disabled when the shell animation setting is off or the OS
  prefers reduced motion.
- Product detail now uses a wide banner with a blurred screenshot backdrop:
  large icon at the far left, title/developer/category in the middle and
  install/open at the right. Summary/stats/trust move to a separate glance strip
  below, and the content cap rises to 1320px with tighter page padding.
- The developer name is a real button. Its developer page reuses the banner
  language and lists every app by that publisher; back-navigation keeps the app →
  developer → app relationship. Evidence: `/home/dev/tmp/store-detail-new.png`,
  `store-developer-new.png`, `store-detail-dark.png`, `store-detail-narrow.png`.
- The Store now consumes the official `AppNavLayout` and `MobileIslandNav` from
  `@rumahl/ui/components` instead of a Store-local island. The draggable active
  pill moves across destinations (commits on release); Arrow/Home/End keys move
  it too. The same layout supplies the desktop rail + content frame and the
  mobile island standard below 760px; the mobile shell also forces the island in
  landscape wider than 760px. Search opens the dedicated page, focuses the field
  and preserves the query through detail navigation. Evidence:
  `/home/dev/tmp/store-island2-mobile.png`, `store-island2-narrow.png`,
  `store-island-drag.png`, `store-search2-mobile.png`,
  `store-search2-results.png`, `store-layout-desktop.png`. The official
  component contract lives in Storybook under `Layout/App/Navbar + sidenav` and
  `Components/Mobile island`.
- Tapping an island destination now uses the native click path, leaving the pill
  free to animate to its target; dragging only starts after a 5px horizontal
  move. RumahlButtonGroup, RumahlTabs and all Settings segmented groups share the
  same animated pill behavior and honor animations-off/reduced-motion. Evidence:
  `/home/dev/tmp/store-island-click-mid.png`, `tabs-mid.png`,
  `display-segments.png`, `store-island-drag.png`.
- The Discover banner's slow drift is now opt-in through Appearance →
  “Animated backgrounds” (the existing `wallpaperMotion` preference, renamed/
  documented to cover app backgrounds). With it off the banner stays still even
  while other animations remain active; reduced-motion and animations-off still
  win. Evidence: `/home/dev/tmp/store-banner-static.png`.
- Shell tests: 54 files / 223 tests passed.
- UI tests: 16 files / 43 tests passed.

This round covered native chrome metrics, frosted page surface, internal
content scrolling and source-driven data selection. Install actions still use
the existing signed-package path; no new installer behavior was changed for the
redesign.


## Discover and product-detail extension (2026-10-26)

Added category app rails, explicit publisher attribution below the title,
optional long descriptions/release notes and same-source developer collections.
Screenshots open the shared UI Lightbox with zoom, thumbnails, arrow navigation,
a failed-image state and native modal focus restoration. The Store now uses its
own window-scoped viewer instead of the OS modal; see the latest Store QA round.

Browser verification used intercepted example-store data, including two images
and optional metadata. Checked desktop Discover/detail views and 390px detail,
gallery and Discover layouts without horizontal document overflow. Verified
arrow navigation, zoom/reset, Escape preserving the detail page and focus
returning to the opener. Fixed the parent Escape handler to respect open modals.
Evidence: `/tmp/store-expanded-detail.jpg`, `/tmp/store-expanded-rails.jpg`,
`/tmp/store-expanded-lightbox.jpg`. Fixture descriptions and developer names are
test data, not shipped catalog content.

Validation: shell/UI TypeScript checks; 17 source/client tests; UI lightbox/modal
tests; Vite production build (existing large-chunk warning). No live package
installation was performed for this visual change.

## Navigation and source model (2026-10-26)

Reviewed the rendered prototype's Discover, catalog, Home Assistant detail and
Registry pages. Retained native store materials; adapted navigation to enabled
source catalogs and local installations. Added full category and source overviews,
source-preserving search, contextual back labels, detail section jumps, and
library detail actions/local search. Product section IDs are per-instance.

Playwright verified two enabled sources plus a disabled source: category browsing,
source status, scoped search, section jumps, returning to results with query intact,
library detail/back, local-only library search, and library visibility after all
sources are removed. Disabled sources were never fetched. Navigation and overviews
fit a 390px viewport without document overflow; no uncaught browser errors.
Captures: `/tmp/store-navigation-sources.jpg`, `/tmp/store-navigation-mobile.jpg`.
Shell TypeScript and production build pass (existing large-chunk warning).

## Store press states and Launcher Settings frame (2026-10-27)

- Store category, app, edit and related cards now press down under the pointer:
  the active state cancels the hover lift (`translateY(1px)`) and scales to
  `0.98`, matching native buttons. Entrance animations no longer keep a filled
  `transform`, so hover/active transforms work afterwards. Evidence:
  `/home/dev/tmp/store-card-pressed.png`.
- Settings opened from Launcher no longer gets a 28px inner frame. The
  launcher/mobile `.shell-window__content { padding: 28px }` rule was more
  specific than the Settings `:has(> .settings-layout)` reset; the reset now
  carries the `.shell` prefix, so the sidebar and content sit flush in the
  window. Evidence: `/home/dev/tmp/settings-launcher-window.png`.

## Settings and Store row press feedback (2026-10-27)

- Settings overview drill-in rows (`Personalization`, `Workspace`, external
  source/license rows), Settings sidebar destinations and external `ButtonLink`
  actions now share the native press feedback. Settings nav rows are full-bleed
  with no inset hover margin and only the pressed row moves down
  (`translateY(1px)`); the surrounding group stays put. Radius follows grouped
  list anatomy: first row rounds only its top corners, last row only its bottom
  corners, and middle rows stay square.
- The Discover “More from your sources” registry rows press down with the same
  scale/translation as the other Store card buttons; standalone category cards
  such as Productivity press as whole rounded surfaces. Evidence:
  `/home/dev/tmp/store-card-pressed.png`, `/home/dev/tmp/settings-row-hover-final.png`,
  `/home/dev/tmp/settings-row-press-final.png`,
  `/home/dev/tmp/settings-launcher-window.png`.

## Files icons and dedicated topbar (2026-10-27)

- Replaced the generic folder drawing with the supplied custom folder SVG
  geometry. Both folder layers are painted from the active theme accent, so the
  artwork follows the primary colour in light and dark themes without fixed
  image assets or third-party artwork.
- Folders in grid, list and empty states use the same artwork at their native
  scale. Desktop app shortcuts keep their app icon; `/apps` packages now resolve
  against the installed app catalogue and show the app identity/icon plus the
  friendly app title instead of a bare package folder name.
- The grid is card-free: icon above a centred name, no panel, border or shadow,
  matching the desktop/launcher icon language. Hover only lifts the artwork;
  selection uses a soft accent wash. Folder/file/app labels no longer repeat
  type or size metadata in icon view.
- The Files app itself now uses the same folder artwork as its app icon on the
  desktop, launcher, taskbar and dock, tinted by the current icon colour.
- List view is now a native table rather than a rounded card: sticky Name,
  Type, Size and Modified headers; 43px rows; folder/app/file artwork; uppercase
  file-extension types; full-row hover and selection; type/modified columns
  collapse on compact windows so Name and Size remain readable.
- Files now renders its own topbar inside the window chrome: Files identity,
  back/forward/up, breadcrumb, new folder/file/upload actions, search and the
  grid/list switch. The previous separate toolbar/address rows are retained only
  as the compact-window fallback below 980px, so narrow and mobile windows keep
  all actions without crowding the topbar.
- Evidence: `/home/dev/tmp/files-before.png`,
  `/home/dev/tmp/files-dark.png`, `/home/dev/tmp/files-folder-crop.png`,
  `/home/dev/tmp/files-list.png`, `/home/dev/tmp/files-list-selected.png`,
  `/home/dev/tmp/files-mobile.png`, `/home/dev/tmp/files-mobile-drawer.png`,
  `/home/dev/tmp/files-desktop-appicon.png`.
- Validation: Shell TypeScript; 54 Shell test files / 224 tests; 16 UI test
  files / 43 tests.
