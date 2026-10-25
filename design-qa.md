# Mobile rumahl OS refinement

final result: passed

## Scope and visual references

User-approved refinement of the existing mobile shell toward the existing tablet launcher and desktop window language. Not a pixel-identical recreation of the old mobile interface.

- Source: public demo https://demo.rumahl.com/, captured 2026-10-24.
- Source captures: `frontend/.shots/mobile-refinement/mobile-{home,settings,windows}.png`, `tablet-home.png`, `tablet-settings.png`, `desktop-settings.png` in the same directory.
- Local implementation: http://localhost:5173/demo.html.
- Implementation captures: `frontend/.shots/mobile-refinement/after-{home,settings,windows}.png`.
- Full-view paired evidence: `frontend/.shots/mobile-refinement/compare-{home,settings,windows}.png`. Each source and implementation image is 440×956 pixels, at 440×956 CSS pixels and DPR 1. Pair renders retain native dimensions. Clock times differ.
- Focus regions: launcher greeting/search spacing, app titlebar/background border, bottom navigation, and switcher card/header. These were inspected in the full-resolution paired captures; additional crops were unnecessary.
- Additional captured viewports: 320×568, 844×390 landscape, 390×844 light, 1024×1366 tablet launcher, 1440×1000 desktop.

## Findings and iterations

1. Original mobile apps removed the system frame and replaced the dock with two unlabeled icons. Restored a compact titlebar, 24px corners, an 8px wallpaper margin, and a dedicated labeled Home/Open windows dock. The content scrolls independently above the dock. Large-device layouts retain their existing frame and dock.
2. Original launcher flattened the tablet greeting hierarchy. Restored a separate display-size name and more separation around search, with a compact variant for short screens. Original app icons, wallpaper and fonts are retained.
3. Original switcher used technical route paths and generic text cards. Replaced with inert copies of already rendered first-party content, app identities, and a horizontally scrollable overview. Sandboxed apps use their app identity as a fallback; their frames are never inspected or duplicated. Entered form values are omitted from previews. Previews are discarded when the switcher closes.
4. First implementation had a P1 intrinsic-width overflow with multiple cards, pushing the heading outside the viewport. Fixed dialog grid tracks and child minimum widths; verified the heading, dismiss action and cards stay inside the viewport in `after-multiple.png`. The currently active card scrolls into view.
5. Initial narrow-screen content retained excessive nested padding (P2). Removed the outer content padding while retaining app-specific content spacing; verified readable rows and reachable navigation at 320×568 in `after-small-settings.png`.
6. Keyboard check identified the native dialog's Tab transition to browser chrome. Added explicit first/last button wrapping, excluding inert preview controls. Verified Tab cycling and Escape dismissal in Chromium.

No remaining actionable P0/P1/P2 findings in the checked states.

## Fidelity surfaces

- Typography: existing theme family and weights retained; name hierarchy restored; mobile navigation labels wrap when needed. Fontconfig fallback was required for browser system fonts. Exact glyph rasterization across real devices is not claimed.
- Spacing/layout: rounded app frame, safe-area-aware dock reserve, bounded overview and scrollable cards. Small screens keep scrollable app content and available persistent controls.
- Color/tokens: overview and navigation use existing panel, text and accent tokens. Both light/dark screenshots inspected; solid-surface and reduced-transparency preferences supported.
- Assets: existing wallpaper, app artwork and icon components reused; no generated replacement assets. Preview UI is a non-interactive copy of authorized, mounted first-party content rather than a second application instance.
- Copy: existing translated Home/Open windows/Close strings reused. Technical paths and redundant Done action removed. Other app copy is outside this change.

## Verification

- Existing shell suite: 49 files / 191 tests passed.
- Added preview regressions: 2 tests passed (no sandbox frame cloning; no actions, duplicate IDs, or entered form values in copies).
- Shell TypeScript check passed.
- ESLint for added/rewritten preview and switcher modules passed.
- Demo Vite production build passed (bundle-size advisory remains).
- Browser flows passed: open apps, Home, app switching, close focused/background apps, empty state, modal keyboard cycle, Escape, frame-count stability while opening switcher, no iframe inside previews, small/landscape/light viewports, tablet and desktop smoke checks.
- Additional check: active card visible on open and tapping the preview restores its app.
- Browser page errors: none in the multi-viewport verification run.
- No Rust code changed; Rust toolchain is unavailable in this environment.

## Limits / follow-up

WebGL was disabled because the headless browser crashed with it enabled, so shader refraction fidelity remains unverified. CSS surfaces and layout were checked. Device safe-area rules are implemented but physical iOS/Android keyboards and screen readers were not tested. Sandboxed external apps intentionally show their app identity rather than a content preview. No full accessibility compliance claim.

## Follow-up: mobile topbar consistency

The mobile-only full-width capsule has been replaced with the existing desktop/tablet island presentation. Left: system mark. Right: search, control centre and compact accent-colour avatar. Touch targets remain at least 44×44 CSS pixels; the avatar uses a transparent border to retain desktop-like visual density inside its larger target. Desktop mode controls and extra menu text remain omitted at phone widths.

- Latest screenshots: `frontend/.shots/mobile-refinement/topbar-{phone,small,light,tablet,desktop}.png`.
- Viewports: 440×956, 320×568, 390×844 light, 1024×1366 tablet launcher, 1440×1000 desktop; DPR 1.
- Checked island separation, viewport containment, 44px phone targets, system/profile menus, control centre, search and Escape. All five viewport checks passed; no browser page errors.
- CSS-only change; demo production build and whitespace checks passed. No extra unit tests added for this presentation-only refinement.
- Browser/WebGL limitations above still apply.
