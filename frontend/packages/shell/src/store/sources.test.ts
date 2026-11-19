import { describe, expect, test } from "vitest";
import type { ShellRequest } from "../snapshot-client";
import { fetchStoreDocument, formatCertificateFingerprint, parseStoreIndex, parseStorePackage, storeImageUrl, UntrustedCertificateError } from "./sources";

const source = { id: "https://store.example/index.json", name: "Example", url: "https://store.example/index.json", enabled: true };

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
