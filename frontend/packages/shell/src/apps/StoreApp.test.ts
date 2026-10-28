import { expect, test, vi } from "vitest";
import { scrollToTop } from "./StoreApp";

test("uses native scrollTo when available", () => {
  const scrollTo = vi.fn();
  scrollToTop({ scrollTo } as unknown as HTMLElement, 48);
  expect(scrollTo).toHaveBeenCalledWith({ top: 48 });
});

test("falls back to scrollTop in environments without scrollTo", () => {
  const element = { scrollTop: 120 } as unknown as HTMLElement;
  scrollToTop(element, 0);
  expect(element.scrollTop).toBe(0);
});
