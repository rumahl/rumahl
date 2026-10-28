# Bundled assets and their licenses

Non-code assets are **not stored in this repository**. They come from the
pinned, multi-license [`rumahl/assets`](https://github.com/rumahl/assets)
repository, fetched into the gitignored `assets/upstream/` directory and copied
into the frontend at build time. See [`assets/README.md`](assets/README.md).

Because the fetched checkout is gitignored, it is outside this repository's
REUSE scope; each asset's license and attribution travel with the checkout.

## Photographic wallpapers — LicenseRef-Unsplash

In `rumahl/assets` under `wallpapers/`:

| File | Photographer | Source | License |
| --- | --- | --- | --- |
| `wallpapers/wallpaper-john-rodenn.jpg` | John Rodenn Castillo | https://unsplash.com/photos/rQqWOHZ96OM | Unsplash License |
| `wallpapers/wallpaper-john-towner.jpg` | John Towner | https://unsplash.com/photos/JgOeRuGD_Y4 | Unsplash License |
| `wallpapers/wallpaper-jonny-james.jpg` | Jonny James | https://unsplash.com/photos/3no88nSvK88 | Unsplash License |
| `wallpapers/monstera.jpg` | Unsplash; **source to confirm** | — | Unsplash License |

These files are **not** project-owned and may **not** be relicensed. Their
license and attribution live in `rumahl/assets`
(`LICENSES/LicenseRef-Unsplash.txt`).

> Open item: the Unsplash License permits commercial use, so these wallpapers do
> not contribute to the intended friction against commercial 1:1
> redistribution. `monstera.jpg` also still needs its photographer and source
> URL confirmed.

## First-party artwork — multi-license (`rumahl/assets`)

Project-owned artwork lives in the same multi-license repository. Assets there
may use different licenses and carry their own `LICENSE*` files; the default for
first-party assets is CC BY-NC-SA 4.0.

## Vendored components with their own notices

| Path | License | Notice |
| --- | --- | --- |
| `frontend/packages/shell/src/glass-engine/` | Apache-2.0 | `LICENSE`, `NOTICE` in that directory |

## Adding an asset

Assets do not belong in this repository. Add them to
[`rumahl/assets`](https://github.com/rumahl/assets) and declare their license in
that repository's `REUSE.toml`. Add a `from -> to` rule to
`scripts/sync-assets.mjs` only if the app needs them at a new location.
