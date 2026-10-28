import { describe, expect, test } from "vitest";
import { resolveShellMode } from "./device";

describe("mobile shell mode", () => {
  test("uses the mobile shell by default in portrait", () => {
    expect(resolveShellMode("desktop", { mobile: true, phone: true, landscape: false }, true)).toBe("mobile");
    expect(resolveShellMode("desktop", { mobile: true, phone: true, landscape: false }, false)).toBe("mobile");
  });

  test("requires landscape and opt-in for the desktop", () => {
    expect(resolveShellMode("desktop", { mobile: true, phone: true, landscape: true }, false)).toBe("mobile");
    expect(resolveShellMode("desktop", { mobile: true, phone: true, landscape: true }, true)).toBe("desktop");
    expect(resolveShellMode("launcher", { mobile: true, phone: true, landscape: true }, true)).toBe("mobile");
  });

  test("leaves desktop devices on their preference and maps mobile to the launcher", () => {
    expect(resolveShellMode("desktop", { mobile: false, phone: false, landscape: false }, false)).toBe("desktop");
    expect(resolveShellMode("launcher", { mobile: false, phone: false, landscape: true }, true)).toBe("launcher");
    expect(resolveShellMode("mobile", { mobile: false, phone: false, landscape: true }, false)).toBe("launcher");
  });

  test("keeps tablets on the launcher unless the desktop was opted into", () => {
    expect(resolveShellMode("desktop", { mobile: true, phone: false, landscape: false }, false)).toBe("launcher");
    expect(resolveShellMode("desktop", { mobile: true, phone: false, landscape: true }, true)).toBe("desktop");
    expect(resolveShellMode("mobile", { mobile: true, phone: false, landscape: false }, false)).toBe("launcher");
  });
});
