import { describe, expect, it } from "vitest";
import { parseStoreApp, parseStoreListing, parseStoreSource } from "./store-client";

const sourceId = "a".repeat(32);
const source = {
  id: sourceId,
  kind: "website",
  location: "https://apps.example.com/store.json",
  title: "Example Store",
  status: "ready",
  error: null,
};
const app = {
  sourceId,
  id: "com.rumahl.examples.notes",
  title: "Notes",
  version: "1.0.0",
  description: "Local notes",
  publisher: "com.rumahl.examples",
  icon: null,
};
describe("app store contracts", () => {
  it("accepts complete and optional metadata", () => {
    expect(parseStoreSource(source)).toEqual({
      id: sourceId,
      kind: "website",
      location: source.location,
      title: source.title,
      status: "ready",
    });
    expect(parseStoreApp(app)).toEqual({
      sourceId,
      id: app.id,
      title: app.title,
      version: app.version,
      description: app.description,
      publisher: app.publisher,
    });
    expect(parseStoreApp({ sourceId, id: app.id, title: app.title, version: app.version })).toMatchObject({
      sourceId,
      id: app.id,
    });
    expect(parseStoreListing({ storeVersion: 1, sources: [source], apps: [app] })).toEqual({
      sources: [parseStoreSource(source)],
      apps: [parseStoreApp(app)],
    });
  });

  it("rejects malformed sources, apps and cross-source references", () => {
    for (const value of [
      { ...source, id: "legacy" },
      { ...source, kind: "ftp" },
      { ...source, status: "loading" },
      { ...source, location: "" },
    ]) expect(() => parseStoreSource(value)).toThrow();
    for (const value of [
      { ...app, id: "not an id" },
      { ...app, version: "" },
      { ...app, sourceId: "b".repeat(31) },
      { ...app, description: 42 },
      { ...app, icon: "x".repeat(2049) },
    ]) expect(() => parseStoreApp(value)).toThrow();
    expect(() => parseStoreListing({ storeVersion: 1, sources: [source], apps: [{ ...app, sourceId: "b".repeat(32) }] })).toThrow();
    expect(() => parseStoreListing({ storeVersion: 2, sources: [], apps: [] })).toThrow();
    expect(() => parseStoreListing({ storeVersion: 1, sources: [{ ...source, id: "a".repeat(32) }, { ...source }], apps: [] })).toThrow();
  });
});
