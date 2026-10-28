import { describe, expect, test } from "vitest";
import { resolveShellMode } from "./device";

describe("mobile shell mode", () => {
  test("always uses the launcher in portrait", () => {
    expect(resolveShellMode("desktop", { mobile: true, landscape: false }, true)).toBe("launcher");
    expect(resolveShellMode("desktop", { mobile: true, landscape: false }, false)).toBe("launcher");
  });

  test("requires landscape and opt-in for the desktop", () => {
    expect(resolveShellMode("desktop", { mobile: true, landscape: true }, false)).toBe("launcher");
    expect(resolveShellMode("desktop", { mobile: true, landscape: true }, true)).toBe("desktop");
    expect(resolveShellMode("launcher", { mobile: true, landscape: true }, true)).toBe("launcher");
  });

  test("leaves non-mobile devices on their preference", () => {
    expect(resolveShellMode("desktop", { mobile: false, landscape: false }, false)).toBe("desktop");
    expect(resolveShellMode("launcher", { mobile: false, landscape: true }, true)).toBe("launcher");
  });
});
