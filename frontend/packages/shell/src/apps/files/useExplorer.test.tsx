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

test("opens personal and host text files in the editor route", async () => {
  state.request.mockImplementation(async (url: string) => {
    if (url.startsWith("/api/v1/files?parent=root")) return { ok: true, json: async () => [file("abc", "notes.txt")] };
    return { ok: true, json: async () => [] };
  });
  const openApp = vi.fn();
  const { result } = renderHook(() => useExplorer(
    state.request as unknown as ShellRequest,
    "device",
    true,
    { openApp }
  ));
  await waitFor(() => expect(result.current.entries).toHaveLength(1));
  act(() => result.current.open(result.current.entries[0]!));
  expect(openApp).toHaveBeenCalledWith("/app/editor?name=notes.txt&id=abc");

  act(() => result.current.navigate(freshHost("system")));
  await waitFor(() => expect(result.current.nav.place.kind).toBe("host"));
  act(() => result.current.open({ key: "etc/rumahl.conf", name: "rumahl.conf", directory: false, size: 1, modified: 0, host: true }));
  expect(openApp).toHaveBeenLastCalledWith("/app/editor?name=rumahl.conf&source=host&area=system&path=etc%2Frumahl.conf");
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

test("exposes real pending/completed feedback and rejects same-tick duplicate writes", async () => {
  let finish: (value: unknown) => void = () => {};
  const pending = new Promise(resolve => { finish = resolve; });
  state.request.mockImplementation(async (_url: string, init?: RequestInit) => init?.method === "POST"
    ? pending : { ok: true, json: async () => [] });
  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", false));
  await waitFor(() => expect(result.current.loading).toBe(false));
  let first: Promise<boolean>;
  let duplicate: Promise<boolean>;
  await act(async () => {
    first = result.current.createFolder("Documents");
    duplicate = result.current.createFolder("Documents");
    await duplicate;
  });
  expect(result.current.busy).toBe(true);
  expect(result.current.operation).toMatchObject({ kind: "createFolder", name: "Documents", phase: "pending" });
  act(() => result.current.refresh());
  expect(result.current.operation?.phase).toBe("pending");
  expect(state.request.mock.calls.filter(([, init]) => init?.method === "POST")).toHaveLength(1);
  await act(async () => { finish({ ok: true }); await first!; });
  expect(result.current.operation).toMatchObject({ kind: "createFolder", name: "Documents", phase: "complete" });
  expect(result.current.busy).toBe(false);
});

test("a failed mutation never reports success and can be retried", async () => {
  state.request.mockImplementation(async (_url: string, init?: RequestInit) => init?.method === "PUT"
    ? { ok: false, status: 409 } : { ok: true, json: async () => [] });
  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", false));
  await waitFor(() => expect(result.current.loading).toBe(false));
  await act(async () => { await result.current.move("one", "notes.txt"); });
  expect(result.current.operation).toBeNull();
  expect(result.current.error).toBe("conflict");
  expect(result.current.busy).toBe(false);
  state.request.mockImplementation(async () => ({ ok: true, json: async () => [] }));
  await act(async () => { await result.current.move("one", "notes.txt"); });
  expect(result.current.operation).toMatchObject({ kind: "move", phase: "complete" });
});

test("refresh failure retains files and reports an error instead of silently failing", async () => {
  state.request.mockResolvedValue({ ok: true, json: async () => [file("one", "notes.txt")] });
  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", false));
  await waitFor(() => expect(result.current.entries).toHaveLength(1));
  state.request.mockRejectedValue(new Error("network"));
  act(() => result.current.refresh());
  await waitFor(() => expect(result.current.error).toBe("unavailable"));
  expect(result.current.entries[0]?.name).toBe("notes.txt");
  expect(result.current.refreshing).toBe(false);
});

test("read-only host views cannot mutate even through direct hook calls", async () => {
  state.request.mockResolvedValue({ ok: true, json: async () => [] });
  const { result } = renderHook(() => useExplorer(state.request as unknown as ShellRequest, "device", true));
  act(() => result.current.navigate(freshHost("system")));
  await waitFor(() => expect(result.current.loading).toBe(false));
  state.request.mockClear();
  await act(async () => { await result.current.removeMany(["one"]); await result.current.relocate("one", "notes.txt", "target"); });
  expect(state.request).not.toHaveBeenCalled();
  expect(result.current.operation).toBeNull();
});
