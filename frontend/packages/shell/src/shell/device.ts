/** Best-effort mobile/coarse-pointer detection (client only). */
export function isMobile(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  if (/android|iphone|ipod|ipad|mobile/i.test(ua)) return true;
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    return window.matchMedia("(pointer: coarse)").matches && window.innerWidth < 900;
  }
  return false;
}
