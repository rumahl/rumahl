import { useEffect, useState } from "react";
import { useShell } from "../shell/ShellContext";

/**
 * Safe mode: when enabled in the recovery settings, third-party apps are kept
 * stopped so a broken app cannot keep destabilising the system.
 */
export function useSafeMode(): boolean {
  const { live } = useShell();
  const [safe, setSafe] = useState(false);
  useEffect(() => {
    if (!live) return;
    let active = true;
    const load = () =>
      void live
        .request("/api/v1/recovery/settings", { credentials: "same-origin", cache: "no-store" })
        .then((response) => (response.ok ? response.json() : null))
        .then((settings) => { if (active && settings) setSafe(settings.safeMode === true); })
        .catch(() => undefined);
    load();
    const timer = setInterval(load, 15_000);
    return () => { active = false; clearInterval(timer); };
  }, [live]);
  return safe;
}
