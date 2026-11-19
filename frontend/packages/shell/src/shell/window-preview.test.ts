import { describe, expect, test } from "vitest";
import { cloneWindowPreview } from "./window-preview";

describe("window previews", () => {
  test("never copies sandboxed app frames", () => {
    const source = document.createElement("div");
    source.innerHTML = '<iframe sandbox="allow-scripts" src="https://app.invalid"></iframe>';
    expect(cloneWindowPreview(source)).toBeNull();
    expect(source.querySelector("iframe")).not.toBeNull();
  });
  test("keeps visible content but strips actions, duplicate IDs and entered values", () => {
    const source = document.createElement("div");
    source.innerHTML = '<h1 id="title">Settings</h1><a href="/settings" onclick="alert(1)">Display</a><input name="secret" value="private"><textarea>private note</textarea><div contenteditable="true">draft</div><script>throw 1</script><p hidden>Hidden</p>';
    const copy = cloneWindowPreview(source)!;
    expect(copy.textContent).toContain("Settings");
    expect(copy.textContent).toContain("Display");
    expect(copy.querySelector("[id], [name], [href], [onclick], script, [hidden], [contenteditable]")).toBeNull();
    expect(copy.querySelector("input")!.value).toBe("");
    expect(copy.textContent).not.toMatch(/private|draft|Hidden/);
    expect(copy.inert).toBe(true);
    expect(copy.getAttribute("aria-hidden")).toBe("true");
    expect(source.querySelector("input")!.value).toBe("private");
  });
});
