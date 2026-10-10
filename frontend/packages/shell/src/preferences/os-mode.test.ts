import { describe, expect, test } from "vitest";
import { parseOsModeSettings } from "./client";
import { osModePolicy } from "./OsMode";

const owner = "00000000-0000-4000-8000-000000000000";

describe("os mode client", () => {
  test("parses settings and resolves the effective mode", () => {
    const settings = parseOsModeSettings({
      settingsVersion: 1,
      ownerId: owner,
      revision: 3,
      user: { osMode: "advanced" },
      device: { osMode: "developer" },
      effective: { osMode: "developer" }
    });
    expect(settings.effective.osMode).toBe("developer");
    expect(settings.user.osMode).toBe("advanced");
  });

  test("rejects inconsistent or unknown values", () => {
    expect(() => parseOsModeSettings({
      settingsVersion: 1,
      ownerId: owner,
      revision: 1,
      user: { osMode: "root" },
      device: { osMode: null },
      effective: { osMode: "root" }
    })).toThrow();
    expect(() => parseOsModeSettings({
      settingsVersion: 1,
      ownerId: owner,
      revision: 1,
      user: { osMode: "guided" },
      device: { osMode: null },
      effective: { osMode: "developer" }
    })).toThrow();
  });

  test("policy exposes developer tools only in developer mode", () => {
    expect(osModePolicy("guided")).toMatchObject({ guided: true, advancedSettings: false, terminal: false });
    expect(osModePolicy("advanced")).toMatchObject({ guided: false, advancedSettings: true, browseSystemFiles: true, terminal: false, ssh: false, webConsole: false });
    expect(osModePolicy("developer")).toMatchObject({ advancedSettings: true, terminal: true, ssh: true, webConsole: true });
  });
});
