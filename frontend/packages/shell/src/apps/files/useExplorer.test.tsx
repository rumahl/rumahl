import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  ensureDesktopFolder: vi.fn(async () => "desktop-folder"),
  request: vi.fn()
}));

vi.mock("../../shell/desktop-files", () => ({ ensureDesktopFolder: state.ensureDesktopFolder }));
vi.mock("../fsClient", () => ({ fetchHostList: vi.fn(async () => []), hostContentUrl: () => "/content" }));

import { useExplorer } from "./useExplorer";
import { freshDesktop } from "./types";
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
