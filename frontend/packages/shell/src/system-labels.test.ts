import { describe, expect, test } from "vitest";
import { buildLabel, osName, versionLabel } from "./system-labels";
import { SHELL_VERSION } from "./version";

describe("system labels", () => {
  test("marks demo sessions without hiding the real identifiers", () => {
    expect(osName(true)).toBe("rumahl OS Demo");
    expect(osName(false)).toBe("rumahl OS");
    expect(buildLabel("shell-abc", true)).toBe("DEMO-shell-abc");
    expect(buildLabel("shell-abc", false)).toBe("shell-abc");
    expect(versionLabel(true)).toBe(`DEMO-${SHELL_VERSION}`);
    expect(versionLabel(false)).toBe(SHELL_VERSION);
  });
});
