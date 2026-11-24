import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { defaultTheme } from "@rumahl/ui/themes";
import { mountSandboxTheme } from "./theme-sandbox";

const createObjectURL = vi.fn((blob: Blob) => {
  void blob;
  return "blob:theme";
});
const revokeObjectURL = vi.fn();
const originalCreate = URL.createObjectURL;
const originalRevoke = URL.revokeObjectURL;

beforeEach(() => {
  createObjectURL.mockClear();
  revokeObjectURL.mockClear();
  Object.defineProperty(URL, "createObjectURL", { value: createObjectURL, configurable: true });
  Object.defineProperty(URL, "revokeObjectURL", { value: revokeObjectURL, configurable: true });
});

afterEach(() => {
  if (originalCreate) Object.defineProperty(URL, "createObjectURL", { value: originalCreate, configurable: true });
  else delete (URL as { createObjectURL?: unknown }).createObjectURL;
  if (originalRevoke) Object.defineProperty(URL, "revokeObjectURL", { value: originalRevoke, configurable: true });
  else delete (URL as { revokeObjectURL?: unknown }).revokeObjectURL;
});

describe("sandboxed theme runtime", () => {
  test("mounts opaque-origin frames with no shell access and cleans them up", async () => {
    const theme = {
      ...defaultTheme,
      id: "com.example.sandbox",
      name: "Sandbox",
      code: "export default function activate() { return () => {}; }",
      runtime: "sandbox" as const,
    };
    const background = document.createElement("div");
    const overlay = document.createElement("div");
    const cleanup = mountSandboxTheme(theme, defaultTheme.tokens, true, background, overlay);
    const frames = [
      ...background.querySelectorAll("iframe"),
      ...overlay.querySelectorAll("iframe"),
    ];
    expect(frames).toHaveLength(2);
    for (const frame of frames) {
      expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
      expect(frame.getAttribute("sandbox")).not.toContain("allow-same-origin");
      expect(frame.className).toBe("theme-sandbox-frame");
      expect(frame.getAttribute("referrerpolicy")).toBe("no-referrer");
      expect(frame.src).toContain("blob:");
    }
    expect(createObjectURL).toHaveBeenCalledTimes(2);
    const html = await (createObjectURL.mock.calls[0]?.[0] as Blob).text();
    expect(html).toContain("Content-Security-Policy");
    expect(html).toContain("connect-src 'none'");
    expect(html).toContain("form-action 'none'");
    expect(html).toContain("rumahl-theme-data");
    expect(html).not.toContain("allow-same-origin");
    cleanup();
    expect(background.children).toHaveLength(0);
    expect(overlay.children).toHaveLength(0);
    expect(revokeObjectURL).toHaveBeenCalledTimes(2);
  });

  test("is a no-op when URL.createObjectURL is unavailable", () => {
    Object.defineProperty(URL, "createObjectURL", { value: undefined, configurable: true });
    const background = document.createElement("div");
    const overlay = document.createElement("div");
    const theme = { ...defaultTheme, code: "export default () => {}", runtime: "sandbox" as const };
    const cleanup = mountSandboxTheme(theme, defaultTheme.tokens, false, background, overlay);
    expect(background.children).toHaveLength(0);
    expect(overlay.children).toHaveLength(0);
    cleanup();
  });
});
