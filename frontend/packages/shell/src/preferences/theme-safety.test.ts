import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { validateTheme } from "./theme-safety";

const base = {
  id: "com.test.theme",
  name: "Test",
  tokens: {
    "color.text.primary": "#111111",
    "color.text.muted": "#767676",
    "color.surface.strong": "#ffffff",
    "color.panel.background": "#ffffff",
    "color.canvas.background": "#ffffff",
    "color.accent": "#111111",
    "color.on.accent": "#ffffff"
  }
};

describe("validateTheme", () => {
  it("accepts a readable theme", () => {
    expect(validateTheme(base).ok).toBe(true);
  });

  it("flags unreadable text", () => {
    const report = validateTheme({
      ...base,
      tokens: { ...base.tokens, "color.text.primary": "#ffffff" }
    });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "contrast")).toBe(true);
  });

  it("flags CSS that disables a critical region", () => {
    const report = validateTheme({ ...base, css: ".shell { display: none }" });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "unsafe-css")).toBe(true);
  });

  it("allows CSS that hides a non-critical element", () => {
    expect(validateTheme({ ...base, css: ".rumahl-badge { display: none }" }).ok).toBe(true);
  });

  it("warns about full-screen fixed layers", () => {
    const report = validateTheme({ ...base, css: ".overlay { position: fixed; inset: 0 }" });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.level === "warning")).toBe(true);
  });

  it("rejects unsafe code", () => {
    const report = validateTheme({ ...base, code: "eval('1')" });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "unsafe-code")).toBe(true);
  });

  it("requires a declared capability for restricted APIs", () => {
    const report = validateTheme({ ...base, code: "fetch('/api')" });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "capability")).toBe(true);
  });

  it("allows a declared capability", () => {
    expect(validateTheme({ ...base, code: "fetch('/api')", capabilities: ["network"] }).ok).toBe(true);
  });

  it("warns about unknown capabilities", () => {
    const report = validateTheme({ ...base, capabilities: ["telepathy"] });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "unknown-capability")).toBe(true);
  });

  it("blocks external resources", () => {
    const report = validateTheme({ ...base, css: "@import url('https://fonts.googleapis.com/x.css');" });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "external-resource")).toBe(true);
  });

  it("allows bundled / data resources", () => {
    expect(validateTheme({ ...base, css: ".a { background: url('data:image/png;base64,AA') }" }).ok).toBe(true);
  });

  it("warns, but does not block, muted text below AA", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "color.text.muted": "#888888" } });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "contrast-muted" && issue.level === "warning")).toBe(true);
  });

  it("blocks muted text below the hard readability floor", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "color.text.muted": "#aaaaaa" } });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "contrast-muted" && issue.level === "error")).toBe(true);
  });

  it("warns about accent ink below AA without breaking legacy themes", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "color.accent": "#111111", "color.on.accent": "#767676" } });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "contrast-on-accent" && issue.level === "warning")).toBe(true);
  });

  it("blocks an accent that disappears into the surface", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "color.accent": "#f8f8f8" } });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "contrast-ui")).toBe(true);
  });

  it("composites translucent surfaces before checking contrast", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "color.surface": "#000000aa" } });
    expect(report.ok).toBe(false);
    expect(report.issues.some((issue) => issue.code === "contrast-muted" && issue.level === "error")).toBe(true);
  });

  it("warns about custom tokens without blocking them", () => {
    const report = validateTheme({ ...base, tokens: { ...base.tokens, "custom.brand.channel": "#123456" } });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "custom-tokens")).toBe(true);
  });

  it("warns about a newer portable schema without blocking", () => {
    const report = validateTheme({ ...base, schemaVersion: 2 });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "schema-version" && issue.level === "warning")).toBe(true);
  });

  it("accepts the current portable schema", () => {
    expect(validateTheme({ ...base, schemaVersion: 1 }).issues.some((issue) => issue.code === "schema-version")).toBe(false);
  });

  it("warns about legacy base names", () => {
    const report = validateTheme({ ...base, base: "clean" });
    expect(report.ok).toBe(true);
    expect(report.issues.some((issue) => issue.code === "legacy-base")).toBe(true);
  });

  it("keeps all bundled theme packages installable", () => {
    const files: string[] = [];
    const walk = (root: string) => {
      for (const entry of readdirSync(root)) {
        const path = join(root, entry);
        if (statSync(path).isDirectory()) walk(path);
        else if (entry.endsWith(".json")) files.push(path);
      }
    };
    walk("themes");
    walk("../../../examples/themes");
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const report = validateTheme(JSON.parse(readFileSync(file, "utf8")));
      expect(report.ok, `${file}: ${report.issues.map((issue) => issue.message).join(" ")}`).toBe(true);
    }
  });
});
