import { describe, expect, test } from "vitest";
import type { ShellRequest } from "../snapshot-client";
import { fetchStoreDocument, fetchStoreIndex, formatCertificateFingerprint, parseGithubSource, parseStoreIndex, parseStorePackage, storeImageUrl, StoreSourceNotFoundError, UntrustedCertificateError, type StoreSource } from "./sources";

const source: StoreSource = { id: "https://store.example/index.json", kind: "website", name: "Example", url: "https://store.example/index.json", enabled: true };

describe("store source protocol", () => {
  test("parses and resolves a store index", () => {
    const index = parseStoreIndex({
      storeVersion: 1,
      name: "Example Store",
      apps: [{ id: "com.example.notes", title: "Notes", version: "1.0.0", summary: "Local notes", publisher: "Example", packageUrl: "./notes.package.json" }]
    }, source);
    expect(index.name).toBe("Example Store");
    expect(index.apps[0]).toMatchObject({ sourceId: source.id, sourceName: "Example", packageUrl: "https://store.example/notes.package.json" });
  });

  test("rejects insecure package URLs and duplicate app ids", () => {
    expect(() => parseStoreIndex({ storeVersion: 1, name: "Bad", apps: [{ id: "com.example.notes", title: "Notes", version: "1", packageUrl: "http://store.example/notes.json" }] }, source)).toThrow();
    expect(() => parseStoreIndex({ storeVersion: 1, name: "Bad", apps: [
      { id: "com.example.notes", title: "Notes", version: "1", packageUrl: "https://store.example/notes.json" },
      { id: "com.example.notes", title: "Notes 2", version: "2", packageUrl: "https://store.example/notes2.json" }
    ] }, source)).toThrow();
  });

  test("accepts a package document and rejects traversal paths", () => {
    expect(parseStorePackage({ packageVersion: 1, files: [{ path: "package.json", content: "e30=" }] })).toHaveLength(1);
    expect(() => parseStorePackage({ packageVersion: 1, files: [{ path: "../package.json", content: "e30=" }] })).toThrow();
    expect(() => parseStorePackage({ packageVersion: 1, files: [{ path: "package.json", content: "not base64!" }] })).toThrow();
  });
});

describe("store certificate trust", () => {
  test("surfaces an explicit certificate decision instead of a generic error", async () => {
    const request = (async () => new Response(JSON.stringify({
      storeError: "untrusted-certificate",
      host: "127.0.0.1",
      fingerprint: "ab".repeat(32),
      algorithm: "sha256"
    }), { status: 428, headers: { "content-type": "application/json" } })) as unknown as ShellRequest;
    await expect(fetchStoreDocument(request, "https://127.0.0.1:8787/store.json", new AbortController().signal))
      .rejects.toBeInstanceOf(UntrustedCertificateError);
    expect(formatCertificateFingerprint("ab".repeat(32))).toMatch(/^AB(:AB){31}$/);
  });
});

describe("store metadata", () => {
  test("parses optional discovery metadata and same-origin artwork", () => {
    const index = parseStoreIndex({
      storeVersion: 1,
      name: "Example Store",
      apps: [{
        id: "com.example.notes", title: "Notes", version: "1.0.0", packageUrl: "notes.package.json",
        category: "Productivity", categories: ["Utilities", "Productivity"],
        rating: 4.76, downloads: 18400, featured: true,
        permissions: ["Read personal files", "Show notifications"],
        iconUrl: "./media/tones-icon.png",
        screenshots: ["/media/notes.png", "https://cdn.example/shot.png", "http://store.example/plain.png"]
      }]
    }, source);
    expect(index.apps[0]).toMatchObject({
      categories: ["Productivity", "Utilities"], rating: 4.8, downloads: 18400, featured: true,
      permissions: ["Read personal files", "Show notifications"],
      iconUrl: "https://store.example/media/tones-icon.png",
      screenshots: ["https://store.example/media/notes.png"]
    });
    expect(storeImageUrl("https://store.example/media/notes.png"))
      .toBe("/api/v1/shell/store/image?url=https%3A%2F%2Fstore.example%2Fmedia%2Fnotes.png");
  });

  test("ignores invalid optional metadata without rejecting the source", () => {
    const index = parseStoreIndex({
      storeVersion: 1,
      name: "Example Store",
      apps: [{
        id: "com.example.notes", title: "Notes", version: "1.0.0", packageUrl: "notes.package.json",
        rating: 8, downloads: -1, featured: "yes", categories: ["Valid", 42, ""],
        iconUrl: "https://cdn.example/icon.png", screenshots: "not-a-list"
      }]
    }, source);
    expect(index.apps[0]).toMatchObject({
      categories: ["Valid"], rating: null, downloads: null, featured: false,
      permissions: [], iconUrl: null, screenshots: []
    });
  });
});

 test("bounds optional product descriptions and ignores non-text release notes", () => {
   const app = { id: "com.example.notes", title: "Notes", version: "1", packageUrl: "./notes.json" };
   const index = parseStoreIndex({ storeVersion: 1, name: "Example", apps: [{ ...app, description: "a".repeat(13000), releaseNotes: { html: "untrusted" } }] }, source);
   expect(index.apps[0]?.description).toHaveLength(12000);
   expect(index.apps[0]?.releaseNotes).toBe("");
   const legacy = parseStoreIndex({ storeVersion: 1, name: "Example", apps: [app] }, source);
   expect(legacy.apps[0]?.description).toBe("");
 });

describe("github store sources", () => {
  test("derives the raw index URL from repository specs", () => {
    expect(parseGithubSource("github:example/apps")).toMatchObject({
      owner: "example", repo: "apps", ref: "main", path: "",
      indexUrl: "https://raw.githubusercontent.com/example/apps/main/store.json"
    });
    expect(parseGithubSource("github:example/apps@dev/catalog")).toMatchObject({
      owner: "example", repo: "apps", ref: "dev", path: "catalog",
      indexUrl: "https://raw.githubusercontent.com/example/apps/dev/catalog/store.json"
    });
    expect(parseGithubSource("https://github.com/example/apps")).toMatchObject({ ref: "main", path: "" });
    expect(parseGithubSource("https://github.com/example/apps/tree/v1/catalog")).toMatchObject({
      ref: "v1", path: "catalog",
      indexUrl: "https://raw.githubusercontent.com/example/apps/v1/catalog/store.json"
    });
  });

  test("rejects other hosts, incomplete specs and path traversal", () => {
    expect(parseGithubSource("https://gitlab.com/example/apps")).toBeNull();
    expect(parseGithubSource("github:onlyowner")).toBeNull();
    expect(parseGithubSource("github:example/apps@dev/../secret")).toBeNull();
    expect(parseGithubSource("https://store.example/index.json")).toBeNull();
  });

  test("honours a development raw base override", () => {
    const globals = globalThis as { __RUMAHL_GITHUB_RAW_BASE__?: unknown };
    globals.__RUMAHL_GITHUB_RAW_BASE__ = "https://127.0.0.1:8787/github";
    try {
      expect(parseGithubSource("github:example/apps")?.indexUrl)
        .toBe("https://127.0.0.1:8787/github/example/apps/main/store.json");
    } finally {
      delete globals.__RUMAHL_GITHUB_RAW_BASE__;
    }
  });

  test("fetches a store index through the bridge", async () => {
    const github: StoreSource = {
      id: "https://raw.githubusercontent.com/example/apps/main/store.json", kind: "github",
      name: "example/apps", url: "https://raw.githubusercontent.com/example/apps/main/store.json", enabled: true
    };
    const request = (async () => new Response(JSON.stringify({
      storeVersion: 1, name: "Example", apps: [
        { id: "com.example.notes", title: "Notes", version: "1.0.0", packageUrl: "packages/notes.json" }
      ]
    }), { status: 200, headers: { "content-type": "application/json" } })) as unknown as ShellRequest;
    const index = await fetchStoreIndex(request, github, new AbortController().signal);
    expect(index.apps[0]).toMatchObject({
      sourceId: github.id,
      packageUrl: "https://raw.githubusercontent.com/example/apps/main/packages/notes.json"
    });
  });

  test("falls back to a single app when the repository has no store.json", async () => {
    const github: StoreSource = {
      id: "https://raw.githubusercontent.com/example/apps/main/store.json", kind: "github",
      name: "example/apps", url: "https://raw.githubusercontent.com/example/apps/main/store.json", enabled: true
    };
    const urls: string[] = [];
    const request = (async (url: string) => {
      urls.push(String(url));
      if (String(url).includes("manifest.json"))
        return new Response(JSON.stringify({
          formatVersion: 1, publisherId: "com.example",
          app: { appId: "com.example.notes", version: "2.1.0", displayName: "Notes" }
        }), { status: 200, headers: { "content-type": "application/json" } });
      return new Response(JSON.stringify({ storeError: "not-found" }), { status: 404, headers: { "content-type": "application/json" } });
    }) as unknown as ShellRequest;
    const index = await fetchStoreIndex(request, github, new AbortController().signal);
    expect(urls[1]).toContain("/api/v1/shell/store/fetch");
    expect(decodeURIComponent(urls[1] ?? "")).toContain("https://raw.githubusercontent.com/example/apps/main/manifest.json");
    expect(index.apps).toEqual([expect.objectContaining({
      id: "com.example.notes", title: "Notes", version: "2.1.0",
      packageUrl: "https://raw.githubusercontent.com/example/apps/main/store-package.json"
    })]);
    await expect(fetchStoreDocument(request, "https://raw.githubusercontent.com/example/apps/missing.json", new AbortController().signal))
      .rejects.toBeInstanceOf(StoreSourceNotFoundError);
  });

  test("keeps website sources without a fallback", async () => {
    const request = (async () => new Response(JSON.stringify({ storeError: "not-found" }), {
      status: 404, headers: { "content-type": "application/json" }
    })) as unknown as ShellRequest;
    await expect(fetchStoreIndex(request, source, new AbortController().signal))
      .rejects.toBeInstanceOf(StoreSourceNotFoundError);
  });
});
