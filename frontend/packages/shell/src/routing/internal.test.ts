import { describe, expect, test } from "vitest";
import { DESIGN_URI, shellLocation } from "./internal";
import { localPath } from "./paths";
import { describeRoute } from "./routes";
import { readInternalWindows, writeInternalWindows } from "../shell/internal-windows";
import { windowBelongsToLink } from "../shell/dock-model";

const design = { id: "internal:design", title: "Design", subtitle: "", location: DESIGN_URI, minimized: true };
describe("internal destinations and dock identity", () => {
  test("round-trips the registered URI without a server route", () => {
    expect(localPath(DESIGN_URI)).toBe("/#rumahl://design");
    expect(shellLocation(localPath(DESIGN_URI))).toBe(DESIGN_URI);
    expect(describeRoute(localPath(DESIGN_URI)).id).toBe("internal:design");
    expect(describeRoute("/_design").presentation).toBe("page");
    expect(() => localPath("rumahl://unknown")).toThrow();
    expect(() => localPath("https://example.com")).toThrow();
  });
  test("persists internal windows only and removes closed entries", () => {
    writeInternalWindows([design, { ...design, location: "/settings" }]);
    expect(readInternalWindows()).toEqual([design]);
    writeInternalWindows([]);
    expect(readInternalWindows()).toEqual([]);
  });
  test("matches pinned route boundaries without swallowing app or hidden windows", () => {
    expect(windowBelongsToLink({ ...design, location: "/settings/display?tab=colors" }, "/settings")).toBe(true);
    expect(windowBelongsToLink({ ...design, location: "/app/files" }, "/apps")).toBe(false);
    expect(windowBelongsToLink({ ...design, location: "/settings-other" }, "/settings")).toBe(false);
    expect(windowBelongsToLink(design, "/settings")).toBe(false);
  });
});
