import { useEffect, useState } from "react";

/** Best-effort mobile/coarse-pointer detection (client only). */
export function isMobile(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  if (/android|iphone|ipod|ipad|mobile/i.test(ua)) return true;
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    // `min` also catches phones and tablets in landscape, where the width is
    // large but the short edge still identifies a touch device.
    return window.matchMedia("(pointer: coarse)").matches && Math.min(window.innerWidth, window.innerHeight) < 900;
  }
  return false;
}

export interface DeviceViewport {
  mobile: boolean;
  landscape: boolean;
}

const MOBILE_DESKTOP_KEY = "rumahl.mobile.desktop";

/** The user explicitly opted into the desktop shell on this mobile browser. */
export function mobileDesktopOptIn(): boolean {
  if (typeof localStorage === "undefined") return false;
  try { return localStorage.getItem(MOBILE_DESKTOP_KEY) === "1"; } catch { return false; }
}

export function setMobileDesktopOptIn(value: boolean): void {
  if (typeof localStorage === "undefined") return;
  try {
    if (value) localStorage.setItem(MOBILE_DESKTOP_KEY, "1");
    else localStorage.removeItem(MOBILE_DESKTOP_KEY);
  } catch { /* private mode */ }
}

export type PreferredShellMode = "desktop" | "launcher";

/**
 * Phone policy: portrait always uses the launcher; landscape may use the
 * desktop, but only after an explicit opt-in. Other devices keep the
 * preference unchanged.
 */
export function resolveShellMode(preferred: PreferredShellMode, viewport: DeviceViewport, desktopOptIn: boolean): PreferredShellMode {
  if (!viewport.mobile) return preferred;
  return viewport.landscape && desktopOptIn ? preferred : "launcher";
}

function readViewport(): DeviceViewport {
  if (typeof window === "undefined") return { mobile: false, landscape: true };
  return { mobile: isMobile(), landscape: window.innerWidth >= window.innerHeight };
}

/**
 * Reactive mobile/landscape state. Starts neutral on the server so hydration
 * stays deterministic and resolves the real device right after mount.
 */
export function useDeviceViewport(): DeviceViewport {
  const [viewport, setViewport] = useState<DeviceViewport>({ mobile: false, landscape: true });
  useEffect(() => {
    const update = () => setViewport(readViewport());
    update();
    window.addEventListener("resize", update);
    window.addEventListener("orientationchange", update);
    const media = typeof window.matchMedia === "function" ? window.matchMedia("(orientation: landscape)") : null;
    media?.addEventListener?.("change", update);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("orientationchange", update);
      media?.removeEventListener?.("change", update);
    };
  }, []);
  return viewport;
}
