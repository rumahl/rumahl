import { expect, test } from "vitest";
import { emptySearchMemory, parseSearchMemory, rememberSearch, resolveSearchMemory, toggleSearchFavorite } from "./search-memory";

test("invalid or untrusted storage cannot become executable targets", () => {
  expect(parseSearchMemory("invalid")).toEqual(emptySearchMemory());
  expect(parseSearchMemory(JSON.stringify({ favorites: [null, 1, "", "typed", "design.rl", "app:files", "app:files"], recent: {} })))
    .toEqual({ favorites: ["app:files"], recent: [] });
});

test("recents are unique, newest first and bounded", () => {
  let memory = emptySearchMemory();
  for (let i = 0; i < 60; i++) memory = rememberSearch(memory, `app:${i}`);
  memory = rememberSearch(memory, "app:30");
  expect(memory.recent).toHaveLength(40);
  expect(memory.recent[0]).toBe("app:30");
  expect(memory.recent.filter(id => id === "app:30")).toHaveLength(1);
  expect(rememberSearch(memory, "typed")).toEqual(memory);
  expect(rememberSearch(memory, "design.rl")).toEqual(memory);
});

test("favorites toggle without changing recent history", () => {
  const memory = rememberSearch(emptySearchMemory(), "app:files");
  const favorite = toggleSearchFavorite(memory, "app:files");
  expect(favorite.favorites).toEqual(["app:files"]);
  expect(toggleSearchFavorite(favorite, "app:files")).toEqual(memory);
  expect(toggleSearchFavorite(memory, "typed")).toEqual(memory);
});

test("stored ids only resolve to currently available, enabled entries", () => {
  const entries = [{ id: "allowed", disabled: false }, { id: "denied", disabled: true }];
  expect(resolveSearchMemory(["removed", "denied", "allowed"], entries)).toEqual([entries[0]]);
  expect(resolveSearchMemory(["allowed"], [])).toEqual([]);
});
