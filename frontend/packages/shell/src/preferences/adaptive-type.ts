import { useEffect, useState } from "react";

/** 100% is calibrated to a 1440 × 900 desktop. */
const REFERENCE_AREA = 1440 * 900;

/**
 * Keeps 100% in a comparable physical range without turning large monitors into
 * oversized UI. The logarithmic curve is intentionally gentle: a full-screen
 * 1440p window is ~5% larger, 4K is capped at 20%, and small windows shrink
 * slightly instead of becoming unreadably small.
 */
export function adaptiveTypographicScale(width: number, height: number): number {
  const area = Math.max(1, width) * Math.max(1, height);
  const ratio = Math.sqrt(area / REFERENCE_AREA);
  const scale = 1 + Math.log2(Math.max(0.25, ratio)) * 0.15;
  return Math.round(Math.min(1.2, Math.max(0.9, scale)) * 100) / 100;
}

/** Uses the actual shell viewport so the scale follows resizes and small windows. */
export function monitorSize(): { width: number; height: number } {
  if (typeof window === "undefined") return { width: 1440, height: 900 };
  return {
    width: window.innerWidth || window.screen?.width || 1440,
    height: window.innerHeight || window.screen?.height || 900,
  };
}

/** Recomputed from the current monitor whenever the window is moved or resized. */
export function useAdaptiveTypeScale(enabled: boolean): number {
  const [scale, setScale] = useState(1);
  useEffect(() => {
    if (!enabled) {
      setScale(1);
      return;
    }
    const update = () => {
      const { width, height } = monitorSize();
      setScale(adaptiveTypographicScale(width, height));
    };
    update();
    const orientation = window.screen?.orientation;
    window.addEventListener("resize", update);
    orientation?.addEventListener("change", update);
    return () => {
      window.removeEventListener("resize", update);
      orientation?.removeEventListener("change", update);
    };
  }, [enabled]);
  return scale;
}
