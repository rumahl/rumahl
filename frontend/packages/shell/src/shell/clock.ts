import { useEffect, useState } from "react";

/** Clock reads the browser clock after mount so SSR markup stays deterministic.
 *  Updates pause while the tab/window is not visible to save work. */
export function useClock(intervalMs = 30_000): Date | null {
  const [now, setNow] = useState<Date | null>(null);
  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const stop = () => { if (timer) { clearInterval(timer); timer = undefined; } };
    const start = () => {
      stop();
      setNow(new Date());
      timer = setInterval(() => { if (typeof document === "undefined" || !document.hidden) setNow(new Date()); }, intervalMs);
    };
    const onVisibility = () => { if (typeof document !== "undefined" && document.hidden) stop(); else start(); };
    start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => { stop(); document.removeEventListener("visibilitychange", onVisibility); };
  }, [intervalMs]);
  return now;
}
