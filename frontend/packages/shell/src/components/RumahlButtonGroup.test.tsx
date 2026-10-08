import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { RumahlButtonGroup } from "./RumahlButtonGroup";

const options = [{ value: "a", label: "Alpha" }, { value: "b", label: "Beta", disabled: true }, { value: "c", label: "Gamma" }];
it("moves keyboard focus past disabled options and selects without submitting", () => {
  const change = vi.fn(), submit = vi.fn();
  render(<form onSubmit={submit}><RumahlButtonGroup label="Mode" options={options} value="a" onChange={change} /></form>);
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
  render(<RumahlButtonGroup label="Format" multiple options={options} value={["a"]} onChange={change} />);
  fireEvent.click(screen.getByRole("button", { name: "Gamma" }));
  expect(change).toHaveBeenLastCalledWith(["a", "c"]);
  fireEvent.click(screen.getByRole("button", { name: "Alpha" }));
  expect(change).toHaveBeenLastCalledWith([]);
  fireEvent.click(screen.getByRole("button", { name: "Beta" }));
  expect(change).toHaveBeenCalledTimes(2);
});
