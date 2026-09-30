import { useEffect, useState } from "react";

/**
 * Resolves the OS light/dark preference. Starts at "light" so the server render
 * and the first client render match, then follows `prefers-color-scheme`.
 */
export function useSystemMode(): "light" | "dark" {
  const [mode, setMode] = useState<"light" | "dark">("light");
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const update = () => setMode(query.matches ? "dark" : "light");
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return mode;
}
