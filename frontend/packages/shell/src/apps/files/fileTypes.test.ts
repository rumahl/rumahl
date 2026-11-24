import { beforeEach, expect, test } from "vitest";
import {
  clearFileAssociation,
  extensionOf,
  fileAssociation,
  resetFileAssociations,
  setFileAssociation
} from "./fileTypes";

beforeEach(() => resetFileAssociations());

test("extracts the lowercase extension of a name", () => {
  expect(extensionOf("Notes.TXT")).toBe("txt");
  expect(extensionOf("archive.tar.gz")).toBe("gz");
  expect(extensionOf("photo.jpeg")).toBe("jpeg");
  expect(extensionOf("README")).toBe("");
  expect(extensionOf(".gitignore")).toBe("");
  expect(extensionOf("trailing.")).toBe("");
});

test("associates a file with a program by extension", () => {
  expect(fileAssociation("notes.txt")?.id).toBe("text");
  expect(fileAssociation("photo.PNG")?.id).toBe("image");
  expect(fileAssociation("clip.mp4")?.id).toBe("media");
  expect(fileAssociation("manual.pdf")?.id).toBe("pdf");
  expect(fileAssociation("notes.txt")?.installed).toBe(true);
  expect(fileAssociation("notes.txt")?.appPath).toBe("/app/editor");
  expect(fileAssociation("README.md")?.id).toBe("text");
  expect(fileAssociation("config.yaml")?.id).toBe("text");
  expect(fileAssociation("main.tsx")?.id).toBe("text");
  expect(fileAssociation("Makefile")?.id).toBe("text");
  expect(fileAssociation(".gitignore")?.id).toBe("text");
  expect(fileAssociation(".env")?.id).toBe("text");
  expect(fileAssociation("unknown.xyz")).toBeUndefined();
});

test("a program can be swapped per extension and reset", () => {
  setFileAssociation("txt", "image");
  expect(fileAssociation("notes.txt")?.id).toBe("image");
  clearFileAssociation("txt");
  expect(fileAssociation("notes.txt")?.id).toBe("text");
});

test("ignores unknown handlers", () => {
  setFileAssociation("txt", "does-not-exist");
  expect(fileAssociation("notes.txt")?.id).toBe("text");
});
