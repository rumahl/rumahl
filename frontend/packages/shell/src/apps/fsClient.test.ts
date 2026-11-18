import { expect, test, vi } from "vitest";
import {
  createHostEntry, parseHostList, parseRumahlPath, PathResolutionError, removeHostEntry,
  renameHostEntry, resolveRumahlPath, writeHostFile
} from "./fsClient";
import type { ShellRequest } from "../snapshot-client";

const entry = { name: "notes.txt", directory: false, size: 3, modified: 1 };
const listing = (area: string, overrides: Record<string, unknown> = {}) => ({
  fsVersion: 1, area, path: "", entries: [entry], ...overrides
});

test("host listings accept every area including the writable home", () => {
  for (const area of ["apps", "system", "home"]) {
    expect(parseHostList(listing(area))).toHaveLength(1);
  }
  expect(() => parseHostList(listing("elsewhere"))).toThrow();
  expect(() => parseHostList(listing("home", { entries: [{ ...entry, name: "../etc" }] }))).toThrow();
  expect(() => parseHostList(listing("home", { entries: [{ ...entry, size: -1 }] }))).toThrow();
});

const request = (status: number) => vi.fn(async () => ({ ok: status < 400, status })) as unknown as ShellRequest;

test("home writes carry the area, device and origin-bound method", async () => {
  const shell = request(204);
  await writeHostFile(shell, "Documents/notes.txt", new Uint8Array([1]), "device", new AbortController().signal);
  const [url, init] = vi.mocked(shell).mock.calls[0] as [string, RequestInit];
  expect(url).toContain("/api/v1/shell/fs/content?");
  expect(url).toContain("area=home");
  expect(url).toContain("path=Documents%2Fnotes.txt");
  expect(init.method).toBe("PUT");
  expect(init.credentials).toBe("same-origin");

  await createHostEntry(shell, "", "Projects", true, "device", new AbortController().signal);
  const create = vi.mocked(shell).mock.calls[1] as [string, RequestInit];
  expect(create[0]).toContain("area=home");
  expect(create[0]).toContain("name=Projects");
  expect(create[0]).toContain("directory=true");
  expect(create[1].method).toBe("POST");

  await renameHostEntry(shell, "Documents", "Docs", "device", new AbortController().signal);
  expect((vi.mocked(shell).mock.calls[2] as [string, RequestInit])[1].method).toBe("PUT");
  await removeHostEntry(shell, "Docs", "device", new AbortController().signal);
  expect((vi.mocked(shell).mock.calls[3] as [string, RequestInit])[1].method).toBe("DELETE");
});

test("host failures map to the explorer's vocabulary", async () => {
  await expect(writeHostFile(request(404), "x", new Uint8Array(), "device", new AbortController().signal))
    .rejects.toThrow("missing");
  await expect(writeHostFile(request(413), "x", new Uint8Array(), "device", new AbortController().signal))
    .rejects.toThrow("limit");
  await expect(removeHostEntry(request(400), "x", "device", new AbortController().signal))
    .rejects.toThrow("conflict");
  await expect(renameHostEntry(request(500), "x", "y", "device", new AbortController().signal))
    .rejects.toThrow("unavailable");
});


test("rumahl paths are canonical across client operating systems", () => {
  expect(parseRumahlPath("rumahl://home/Documents/notes.txt")).toEqual({
    canonical: "rumahl://home/Documents/notes.txt", area: "home", relative: "Documents/notes.txt"
  });
  expect(parseRumahlPath("~/Documents")).toEqual({ canonical: "rumahl://home/Documents", area: "home", relative: "Documents" });
  expect(parseRumahlPath("  HOME\\Documents  ")).toEqual({ canonical: "rumahl://home/Documents", area: "home", relative: "Documents" });
  expect(parseRumahlPath("files")).toEqual({ canonical: "rumahl://home", area: "home", relative: "" });
  expect(parseRumahlPath("desktop")).toEqual({ canonical: "rumahl://desktop", area: "desktop", relative: "" });
  expect(parseRumahlPath("rumahl://apps/com.example.notes/data")).toEqual({ canonical: "rumahl://apps/com.example.notes/data", area: "apps", relative: "com.example.notes/data" });
  // No POSIX host paths, drive letters, traversal or unknown areas.
  for (const invalid of ["/home/alice", "C:\\Users\\alice", "rumahl://home/../etc", "rumahl://desktop/Documents", "rumahl:https://x", "documents", ""]) {
    expect(parseRumahlPath(invalid), invalid).toBeNull();
  }
});

const jsonRequest = (status: number, body: unknown) =>
  vi.fn(async () => ({ ok: status < 400, status, json: async () => body })) as unknown as ShellRequest;
const rumahlPath = parseRumahlPath("rumahl://home/Documents")!;

test("the service resolves a rumahl path and decides folder versus file", async () => {
  const shell = jsonRequest(200, {
    fsVersion: 1, path: "rumahl://home/Documents", area: "home", relative: "Documents",
    name: "Documents", directory: true, writable: true
  });
  const resolved = await resolveRumahlPath(shell, rumahlPath, "device", new AbortController().signal);
  expect(resolved).toEqual({
    canonical: "rumahl://home/Documents", area: "home", relative: "Documents",
    name: "Documents", directory: true, writable: true
  });
  const [url] = vi.mocked(shell).mock.calls[0] as [string, RequestInit];
  expect(url).toContain("/api/v1/shell/fs/resolve?");
  expect(url).toContain("path=rumahl%3A%2F%2Fhome%2FDocuments");
  expect(url).toContain("device=device");
});

test("path resolution failures stay in the explorer's vocabulary", async () => {
  for (const [status, reason] of [[400, "invalid"], [403, "denied"], [404, "missing"], [503, "unavailable"]] as const) {
    const failure = await resolveRumahlPath(jsonRequest(status, {}), rumahlPath, "device", new AbortController().signal)
      .catch((cause: unknown) => cause);
    expect(failure).toBeInstanceOf(PathResolutionError);
    expect((failure as PathResolutionError).reason).toBe(reason);
  }
  await expect(resolveRumahlPath(jsonRequest(200, { fsVersion: 2 }), rumahlPath, "device", new AbortController().signal))
    .rejects.toThrow("unavailable");
});
