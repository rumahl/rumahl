import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { RumahlButtonGroup } from "./RumahlButtonGroup";

const options = [
  { value: "a", label: "Alpha" },
  { value: "b", label: "Beta", disabled: true },
  { value: "c", label: "Gamma" },
];
it("moves keyboard focus past disabled options and selects without submitting", () => {
  const change = vi.fn(),
    submit = vi.fn();
  render(
    <form onSubmit={submit}>
      <RumahlButtonGroup
        label="Mode"
        options={options}
        value="a"
        onChange={change}
      />
    </form>,
  );
  const alpha = screen.getByRole("button", { name: "Alpha" });
  expect(alpha).toHaveAttribute("aria-pressed", "true");
  alpha.focus();
  fireEvent.keyDown(alpha, { key: "ArrowRight" });
  const gamma = screen.getByRole("button", { name: "Gamma" });
  expect(gamma).toHaveFocus();
  fireEvent.click(gamma);
  expect(change).toHaveBeenCalledWith("c");
  expect(submit).not.toHaveBeenCalled();
  fireEvent.keyDown(gamma, { key: "Home" });
  expect(alpha).toHaveFocus();
});
it("toggles multiple values independently", () => {
  const change = vi.fn();
  render(
    <RumahlButtonGroup
      label="Format"
      multiple
      options={options}
      value={["a"]}
      onChange={change}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Gamma" }));
  expect(change).toHaveBeenLastCalledWith(["a", "c"]);
  fireEvent.click(screen.getByRole("button", { name: "Alpha" }));
  expect(change).toHaveBeenLastCalledWith([]);
  fireEvent.click(screen.getByRole("button", { name: "Beta" }));
  expect(change).toHaveBeenCalledTimes(2);
});
it("runs connected actions without reporting a selected state", () => {
  const action = vi.fn();
  render(
    <RumahlButtonGroup
      mode="actions"
      label="Navigation"
      options={options}
      onChange={action}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Gamma" }));
  expect(action).toHaveBeenCalledWith("c");
  expect(screen.getByRole("button", { name: "Gamma" })).not.toHaveAttribute(
    "aria-pressed",
  );
});

it("previews a dragged selection and commits once on release", () => {
  const change = vi.fn();
  render(<RumahlButtonGroup label="Mode" options={options} value="a" onChange={change} />);
  const group = screen.getByRole("group");
  const alpha = screen.getByRole("button", { name: "Alpha" });
  const gamma = screen.getByRole("button", { name: "Gamma" });
  vi.spyOn(alpha, "getBoundingClientRect").mockReturnValue({ left: 0, right: 50, top: 0, bottom: 30 } as DOMRect);
  vi.spyOn(gamma, "getBoundingClientRect").mockReturnValue({ left: 100, right: 150, top: 0, bottom: 30 } as DOMRect);
  function pointer(type: string, x: number) {
    const event = new MouseEvent(type, { bubbles: true, button: 0, clientX: x, clientY: 15 });
    Object.defineProperty(event, "pointerId", { value: 1 });
    fireEvent(group, event);
  }
  pointer("pointerdown", 20);
  pointer("pointermove", 120);
  expect(gamma).toHaveAttribute("data-drag-preview", "true");
  expect(change).not.toHaveBeenCalled();
  pointer("pointerup", 120);
  fireEvent.click(gamma, { detail: 1 });
  expect(change).toHaveBeenCalledExactlyOnceWith("c");
  expect(gamma).toHaveFocus();
  pointer("pointerdown", 20);
  pointer("pointermove", 120);
  pointer("pointercancel", 120);
  expect(group).not.toHaveAttribute("data-dragging");
  expect(change).toHaveBeenCalledTimes(1);
});
