import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { RumahlSearch, RumahlStepper, RumahlTabs } from "./RumahlOsControls";

it("clears search and returns focus to the field", () => {
  function Search() { const [value, setValue] = useState("files"); return <RumahlSearch label="Search" clearLabel="Clear" value={value} onChange={setValue} />; }
  render(<Search />);
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  expect(screen.getByRole("searchbox")).toHaveValue("");
  expect(screen.getByRole("searchbox")).toHaveFocus();
});
it("clamps fractional steps at the boundary", () => {
  const change = vi.fn();
  render(<RumahlStepper label="Value" decreaseLabel="Less" increaseLabel="More" min={0} max={1} step={0.2} value={0.9} onChange={change} />);
  fireEvent.click(screen.getByRole("button", { name: "More" }));
  expect(change).toHaveBeenLastCalledWith(1);
  fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "-3" } });
  expect(change).toHaveBeenLastCalledWith(0);
});
it("switches tab panels with arrow keys and skips disabled tabs", () => {
  function Tabs() {
    const [value, setValue] = useState("a");
    return <RumahlTabs label="Settings" value={value} onChange={setValue} tabs={[
      { id: "a", label: "General", content: "General panel" },
      { id: "b", label: "Disabled", content: "Disabled panel", disabled: true },
      { id: "c", label: "Network", content: "Network panel" },
    ]} />;
  }
  render(<Tabs />);
  fireEvent.keyDown(screen.getByRole("tab", { name: "General" }), { key: "ArrowRight" });
  expect(screen.getByRole("tab", { name: "Network" })).toHaveFocus();
  expect(screen.getByRole("tab", { name: "Network" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tabpanel")).toHaveTextContent("Network panel");
});
