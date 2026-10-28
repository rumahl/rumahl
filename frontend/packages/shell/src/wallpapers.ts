import monstera from "./assets/monstera.jpg";
import johnRoden from "./assets/wallpaper-john-rodenn.jpg";
import johnTowner from "./assets/wallpaper-john-towner.jpg";
import jonnyJames from "./assets/wallpaper-jonny-james.jpg";

interface Photo { url: string; tone: string }

/**
 * Bundled photographic wallpapers. `default` and the photo sentinels are
 * resolved to real URLs at runtime; `tone` approximates the picture's dominant
 * colour for palette extraction and tinting (photographs have no declarative
 * colours to sample synchronously).
 */
const PHOTOS: { [key: string]: Photo } = {
  default: { url: monstera, tone: "#0c1619" },
  "photo:rodenn": { url: johnRoden, tone: "#202225" },
  "photo:towner": { url: johnTowner, tone: "#202225" },
  "photo:jonny": { url: jonnyJames, tone: "#202225" }
};

function find(value: string): Photo | undefined {
  const direct = PHOTOS[value];
  if (direct) return direct;
  return Object.values(PHOTOS).find((photo) => value === `url(${photo.url})`);
}

/** Maps a wallpaper token value ("default", "photo:…") to a CSS image value. */
export function resolveWallpaper(value: string): string {
  const photo = find(value);
  return photo ? `url(${photo.url})` : value;
}

/** Dominant-colour approximation for a bundled photograph, if it is one. */
export function photoTone(value: string): string | null {
  return find(value)?.tone ?? null;
}
