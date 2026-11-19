import { beforeEach, expect, test } from "vitest";
import { clearSessionSnapshot, createSessionSnapshot, flushSessionSnapshot, loadSessionSnapshot, normalizeSessionSnapshot, stageSessionSnapshot } from "./session-snapshot";
import type { ShellWindow } from "../shell-state";

const windows: ShellWindow[] = [{
  id: "app:files",
  title: "Files",
  subtitle: "",
  location: "/app/files",
  minimized: false,
  rect: { x: 20, y: 30, width: 700, height: 500 },
  placement: "floating",
}];

beforeEach(() => {
  clearSessionSnapshot();
  localStorage.clear();
});

test("encrypts the live desktop and restores it for the same account", async () => {
  stageSessionSnapshot(createSessionSnapshot("Developer", windows, "app:files", "/app/files"));
  expect(await flushSessionSnapshot()).toBe(true);
  const raw = localStorage.getItem("rumahl.session.snapshot.v1") ?? "";
  expect(raw).not.toContain("Files");
  expect(raw).not.toContain("/app/files");
  expect(await loadSessionSnapshot("Developer")).toMatchObject({
    account: "Developer",
    focusedWindowId: "app:files",
    activeLocation: "/app/files",
    windows: [{ id: "app:files", title: "Files", location: "/app/files" }],
  });
});

test("never restores a snapshot into another account or with invalid windows", async () => {
  stageSessionSnapshot(createSessionSnapshot("Developer", windows, null, null));
  await flushSessionSnapshot();
  expect(await loadSessionSnapshot("Other user")).toBeNull();
  expect(normalizeSessionSnapshot({ version: 1, account: "Developer", savedAt: Date.now(), windows: [{ id: "x" }], focusedWindowId: null, activeLocation: null }, "Developer")).toBeNull();
});
