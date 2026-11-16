import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  ensureDesktopFolder: vi.fn(async () => "desktop-folder"),
  request: vi.fn()
}));

vi.mock("../../shell/desktop-files", () => ({ ensureDesktopFolder: state.ensureDesktopFolder }));
vi.mock("../fsClient", () => ({ fetchHostList: vi.fn(async () => []), hostContentUrl: () => "/content" }));

import { useExplorer } from "./useExplorer";
import { freshDesktop, freshHost } from "./types";
import type { ShellRequest } from "../../snapshot-client";

const file = (id: string, name: string) => ({ id, name, directory: false, size: 1, modified: 0 });

beforeEach(() => {
  state.ensureDesktopFolder.mockClear();
  state.request.mockReset();
});

test("refreshes the desktop listing without clearing it or showing the loader", async () => {
  let resolveRefresh: (value: unknown) => void = () => {};
  const refreshResponse = new Promise((resolve) => { resolveRefresh = resolve; });
  let listing = 0;
  state.request.mockImplementation(async (url: string) => {
    if (url.startsWith("/api/v1/files?parent=desktop-folder")) {
      listing += 1;
      return {
        ok: true,
        json: async () => (listing === 1 ? [file("one", "one.txt")] : refreshResponse)
      };
    }
    return { ok: true, json: async () => [] };
  });

  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", false));
  act(() => result.current.navigate(freshDesktop()));
  await waitFor(() => expect(result.current.entries).toHaveLength(1));
  expect(result.current.loading).toBe(false);

  act(() => result.current.refresh());
  expect(result.current.loading).toBe(false);
  expect(result.current.entries.map((entry) => entry.name)).toEqual(["one.txt"]);

  await act(async () => { resolveRefresh([file("one", "one.txt"), file("two", "two.txt")]); });
  await waitFor(() => expect(result.current.entries).toHaveLength(2));
  expect(result.current.loading).toBe(false);
});


test("desktop shows app folders and writes real files into the desktop folder", async () => {
  state.request.mockImplementation(async (url: string, init?: { method?: string }) => {
    if (url.startsWith("/api/v1/files?parent=desktop-folder")) return { ok: true, json: async () => [] };
    if (init?.method === "POST") return { ok: true, json: async () => ({}) };
    return { ok: true, json: async () => [] };
  });
  const apps = [{
    id: "files", title: "Files", path: "/app/files", launchable: true, system: true, kind: "app" as const
  }];
  const folders = [{ id: "tools", name: "Tools", apps: ["files"] }];
  const { result } = renderHook(() => useExplorer(
    state.request as unknown as ShellRequest,
    "device",
    false,
    { desktop: { apps, folders }, desktopKey: "desktop-test" }
  ));
  act(() => result.current.navigate(freshDesktop()));
  await waitFor(() => expect(result.current.isWritable).toBe(true));
  await waitFor(() => expect(result.current.entries.some((entry) => entry.appFolder && entry.name === "Tools")).toBe(true));

  await act(async () => { await result.current.createFolder("New Folder"); });
  expect(state.request).toHaveBeenCalledWith(
    expect.stringContaining("parent=desktop-folder"),
    expect.objectContaining({ method: "POST" })
  );
});

test("returns a very large listing for client-side pagination", async () => {
  const files = Array.from({ length: 1005 }, (_, index) => file(`id-${index}`, `file-${index}.txt`));
  state.request.mockImplementation(async (url: string) => {
    if (url.startsWith("/api/v1/files?parent=root")) return { ok: true, json: async () => files };
    return { ok: true, json: async () => [] };
  });
  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", false));
  await waitFor(() => expect(result.current.entries).toHaveLength(1005));
});

test("leaves a host location when advanced mode is demoted", async () => {
  state.request.mockImplementation(async () => ({ ok: true, json: async () => [] }));
  const { result, rerender } = renderHook(
    ({ advanced }: { advanced: boolean }) => useExplorer(state.request as unknown as ShellRequest, "device", advanced),
    { initialProps: { advanced: true } }
  );
  act(() => result.current.navigate(freshHost("apps")));
  await waitFor(() => expect(result.current.nav.place.kind).toBe("host"));
  await waitFor(() => expect(result.current.loading).toBe(false));

  rerender({ advanced: false });
  await waitFor(() => expect(result.current.nav.place.kind).toBe("store"));
  expect(result.current.entries).toEqual([]);
});
