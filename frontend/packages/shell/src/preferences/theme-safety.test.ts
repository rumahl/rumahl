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
});
