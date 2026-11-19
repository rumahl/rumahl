import { describe, expect, test } from "vitest";
import { applyOrder, moveInOrder, paginate } from "./launcher-order";

describe("launcher order", () => {
  test("merges the stored order with the known apps, appending new ones", () => {
    expect(applyOrder(["a", "b", "c"], ["c", "a"])).toEqual(["c", "a", "b"]);
    expect(applyOrder(["a", "b"], ["x", "b", "a"])).toEqual(["b", "a"]);
    expect(applyOrder(["a", "b"], [])).toEqual(["a", "b"]);
    expect(applyOrder(["a", "b", "c"], ["b", "b"])).toEqual(["b", "a", "c"]);
  });

  test("moves an item, clamping the target index", () => {
    expect(moveInOrder(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
    expect(moveInOrder(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
    expect(moveInOrder(["a", "b", "c"], 1, 99)).toEqual(["a", "c", "b"]);
    expect(moveInOrder(["a", "b"], 5, 0)).toEqual(["a", "b"]);
  });

  test("paginates and always returns at least one page", () => {
    expect(paginate(["a", "b", "c", "d", "e"], 2)).toEqual([["a", "b"], ["c", "d"], ["e"]]);
    expect(paginate([], 4)).toEqual([[]]);
    expect(paginate(["a"], 0)).toEqual([["a"]]);
  });
});
