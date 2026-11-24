import { useEffect, useState } from "react";
import type { ShellLiveSource } from "../live-updates";
import type { ShellRequest } from "../snapshot-client";

/** Reads the device-wide full-access theme policy. Missing/denied is false. */
export async function fetchFullAccessThemes(
  request: ShellRequest,
  signal?: AbortSignal,
): Promise<boolean> {
  const response = await request("/api/v1/shell/theme-policy", {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    ...(signal ? { signal } : {}),
  });
  if (!response.ok) return false;
  const value = await response.json() as { allowFullAccessThemes?: unknown };
  return value.allowFullAccessThemes === true;
}

/**
 * Full access is unavailable when the device policy forbids it or the policy
 * cannot be read. Demo shells (no live source) keep the local consent model.
 */
export function useFullAccessThemes(live: ShellLiveSource | undefined): boolean {
  const [allowed, setAllowed] = useState(true);
  useEffect(() => {
    if (!live) {
      setAllowed(true);
      return;
    }
    const controller = new AbortController();
    let active = true;
    void fetchFullAccessThemes(live.request, controller.signal)
      .then((value) => { if (active) setAllowed(value); })
      .catch(() => { if (active) setAllowed(false); });
    return () => {
      active = false;
      controller.abort();
    };
  }, [live]);
  return allowed;
}
