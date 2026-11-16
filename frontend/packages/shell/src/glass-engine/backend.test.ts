import { describe, expect, test } from "vitest";
import { resolveBackendRenderer } from "./engine.js";

describe("glass backend resolution", () => {
  test("an SVG surface never falls back to WebGL over app content", () => {
    expect(resolveBackendRenderer("svg", { svgAvailable: true, webglAvailable: true })).toBe("svg");
    expect(resolveBackendRenderer("svg", { svgAvailable: false, webglAvailable: true })).toBe("css");
  });

  test("explicit WebGL and CSS requests keep their own fallbacks", () => {
    expect(resolveBackendRenderer("webgl", { webglAvailable: true })).toBe("webgl");
    expect(resolveBackendRenderer("webgl", { webglAvailable: false })).toBe("css");
    expect(resolveBackendRenderer("css", { webglAvailable: true })).toBe("css");
  });

  test("reduced transparency always wins", () => {
    expect(resolveBackendRenderer("svg", { svgAvailable: true, reduced: true })).toBe("css");
    expect(resolveBackendRenderer("webgl", { webglAvailable: true, reduced: true })).toBe("css");
  });
});
