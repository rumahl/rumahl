import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { MobileIslandNav, type MobileIslandNavItem } from "./MobileIslandNav";

afterEach(cleanup);

const items: MobileIslandNavItem[] = [
  { id: "a", label: "Alpha", icon: <span>A</span> },
  { id: "b", label: "Beta", icon: <span>B</span> },
  { id: "c", label: "Gamma", icon: <span>C</span> }
];

function geometry() {
  const buttons = screen.getAllByRole("button");
  const lefts = [0, 60, 120];
  buttons.forEach((button, index) => {
    Object.defineProperty(button, "offsetLeft", { configurable: true, value: lefts[index] });
    Object.defineProperty(button, "offsetWidth", { configurable: true, value: 50 });
    Object.defineProperty(button, "offsetTop", { configurable: true, value: 6 });
    Object.defineProperty(button, "offsetHeight", { configurable: true, value: 48 });
  });
  return buttons;
}

it("previews a dragged active indicator and commits once on release", () => {
  const change = vi.fn();
  render(<MobileIslandNav items={items} value="a" onChange={change} label="Sections" />);
  const nav = screen.getByRole("navigation", { name: "Sections" });
  const buttons = geometry();
  function pointer(type: string, x: number) {
    const event = new MouseEvent(type, { bubbles: true, button: 0, clientX: x, clientY: 20 });
    Object.defineProperty(event, "pointerId", { value: 1 });
    fireEvent(nav, event);
  }
  pointer("pointerdown", 20);
  pointer("pointermove", 80);
  expect(buttons[1]!).toHaveAttribute("aria-current", "page");
  expect(buttons[1]!).toHaveAttribute("data-drag-preview", "true");
  expect(change).not.toHaveBeenCalled();
  pointer("pointerup", 80);
  fireEvent.click(buttons[1]!, { detail: 1 });
  expect(change).toHaveBeenCalledExactlyOnceWith("b");
});

it("moves keyboard selection and focus with arrow keys", async () => {
  const change = vi.fn();
  render(<MobileIslandNav items={items} value="a" onChange={change} label="Sections" />);
  geometry();
  const alpha = screen.getByRole("button", { name: "Alpha" });
  alpha.focus();
  fireEvent.keyDown(alpha, { key: "ArrowRight" });
  expect(change).toHaveBeenLastCalledWith("b");
  await waitFor(() => expect(screen.getByRole("button", { name: "Beta" })).toHaveFocus());
  fireEvent.keyDown(screen.getByRole("button", { name: "Beta" }), { key: "End" });
  expect(change).toHaveBeenLastCalledWith("c");
});
