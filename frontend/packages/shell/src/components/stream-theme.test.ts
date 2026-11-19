import { describe, expect, test, vi } from "vitest";
import type { BridgeTheme } from "@rumahl/contracts/bridge";
import { applyStreamTheme } from "./stream-theme";

const theme: BridgeTheme = {
  scheme: "dark", accent: "#28694c", reducedMotion: false,
  palette: { background: "#0f172a", surface: "#1e293b", text: "#e2e8f0", textMuted: "#94a3b8", border: "#334155", accent: "#28694c", onAccent: "#ffffff" }
};

describe("applyStreamTheme", () => {
  test("projects the theme into a same-origin stream and keeps a single style element", () => {
    const iframe = document.createElement("iframe");
    document.body.appendChild(iframe);
    const listener = vi.fn();
    iframe.contentDocument!.addEventListener("rumahl:theme", listener);

    applyStreamTheme(iframe, theme);
    const doc = iframe.contentDocument!;
    expect(doc.documentElement.dataset.rumahlTheme).toBe("dark");
    expect(doc.documentElement.style.getPropertyValue("color-scheme")).toBe("dark");
    const style = doc.getElementById("rumahl-stream-theme") as HTMLStyleElement;
    expect(style.textContent).toContain("--rumahl-accent:#28694c");
    expect((iframe.contentWindow as unknown as { rumahlTheme: BridgeTheme }).rumahlTheme).toEqual(theme);
    expect(listener).toHaveBeenCalledTimes(1);

    applyStreamTheme(iframe, { ...theme, scheme: "light", palette: { ...theme.palette, background: "#ffffff" } });
    expect(doc.querySelectorAll("#rumahl-stream-theme").length).toBe(1);
    expect(doc.documentElement.dataset.rumahlTheme).toBe("light");
    expect(doc.getElementById("rumahl-stream-theme")!.textContent).toContain("--rumahl-background:#ffffff");
    iframe.remove();
  });

  test("ignores a missing iframe", () => {
    expect(() => applyStreamTheme(null, theme)).not.toThrow();
  });
});
