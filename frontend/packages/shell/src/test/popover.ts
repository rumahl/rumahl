import { afterAll, beforeAll, vi } from "vitest";

// jsdom has no top layer or layout; real positioning/dismissal is checked in Chromium.
const open = new WeakSet<HTMLElement>();
const matches = HTMLElement.prototype.matches;
beforeAll(() => {
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  Object.defineProperty(HTMLElement.prototype, "showPopover", { configurable: true, value() { open.add(this); this.style.display = "block"; } });
  Object.defineProperty(HTMLElement.prototype, "hidePopover", { configurable: true, value() { open.delete(this); this.style.display = "none"; } });
  vi.spyOn(HTMLElement.prototype, "matches").mockImplementation(function (this: HTMLElement, selector) {
    return selector === ":popover-open" ? open.has(this) : matches.call(this, selector);
  });
});
afterAll(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(HTMLElement.prototype, "showPopover");
  Reflect.deleteProperty(HTMLElement.prototype, "hidePopover");
});
