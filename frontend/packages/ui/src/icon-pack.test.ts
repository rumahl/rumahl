import { describe, expect, test } from "vitest";
import { readIconPack, sanitizeIconPack, type IconPackInput } from "./icon-pack";
import { defineTheme } from "./sdk";
import { themeFromPackage } from "./themes";

const search = {
  viewBox: "0 0 24 24",
  shapes: [
    { tag: "rect", attrs: { x: 3, y: 3, width: 14, height: 14, rx: 1 } },
    { tag: "path", attrs: { d: "M17 17l4 4" } }
  ]
} satisfies IconPackInput["search"];

describe("theme icon packs", () => {
  test("keeps a valid icon and defaults the viewBox", () => {
    const report = readIconPack({ search: { shapes: search.shapes } });
    expect(report.errors).toEqual([]);
    expect(report.warnings).toEqual([]);
    const icon = report.icons.search;
    expect(icon?.viewBox).toBe("0 0 24 24");
    expect(icon?.filled).toBe(false);
    expect(icon?.shapes).toHaveLength(2);
    expect(icon?.shapes[0]?.attrs).toEqual({ x: 3, y: 3, width: 14, height: 14, rx: 1 });
  });

  test("converts dashed presentation attributes to camelCase", () => {
    const report = readIconPack({
      settings: {
        shapes: [
          {
            tag: "circle",
            attrs: { cx: 12, cy: 12, r: 4, "stroke-width": 2, "fill-rule": "evenodd" }
          }
        ]
      }
    });
    expect(report.errors).toEqual([]);
    expect(report.icons.settings?.shapes[0]?.attrs).toEqual({
      cx: 12,
      cy: 12,
      r: 4,
      strokeWidth: 2,
      fillRule: "evenodd"
    });
  });

  test("rejects scripts, links, images and event attributes", () => {
    for (const shapes of [
      [{ tag: "script", attrs: {} }],
      [{ tag: "image", attrs: { href: "https://example.com/a.svg" } }],
      [{ tag: "path", attrs: { d: "M0 0", onclick: "alert(1)" } }],
      [{ tag: "path", attrs: { d: "M0 0", fill: "url(https://example.com/x.svg)" } }]
    ]) {
      const report = readIconPack({ search: { shapes } });
      expect(report.icons.search).toBeUndefined();
      expect(report.errors.length).toBeGreaterThan(0);
    }
  });

  test("rejects oversized paths and invalid viewBoxes", () => {
    const report = readIconPack({
      search: { viewBox: "0 0 24", shapes: search.shapes },
      grid: { shapes: [{ tag: "path", attrs: { d: `M0 0${" L1 1".repeat(2000)}` } }] }
    });
    expect(report.icons.search).toBeUndefined();
    expect(report.icons.grid).toBeUndefined();
    expect(report.errors).toHaveLength(2);
  });

  test("drops an unsupported icon but keeps the rest", () => {
    const report = readIconPack({ search, bogus: search });
    expect(report.icons.search).toBeDefined();
    expect((report.icons as Record<string, unknown>).bogus).toBeUndefined();
    expect(report.warnings.some((message) => message.includes("bogus"))).toBe(true);
  });

  test("keeps nested groups within the depth limit", () => {
    const report = readIconPack({
      home: {
        filled: true,
        shapes: [
          {
            tag: "g",
            attrs: { opacity: 0.8 },
            children: [{ tag: "path", attrs: { d: "M4 4h16v16H4z" } }]
          }
        ]
      }
    });
    expect(report.icons.home?.filled).toBe(true);
    expect(report.icons.home?.shapes[0]?.children).toHaveLength(1);
  });

  test("defineTheme and theme packages carry the sanitized pack", () => {
    const defined = defineTheme({ id: "com.test.icons", name: "Icons", icons: { search } });
    expect(defined.icons?.search?.shapes).toHaveLength(2);

    const packaged = themeFromPackage({ id: "com.test.packaged", name: "Packaged", icons: { search } });
    expect(packaged.icons?.search?.shapes).toHaveLength(2);

    expect(sanitizeIconPack({ search }).search?.shapes).toHaveLength(2);
  });
});
