import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { GroupContainer, GroupFooter, GroupRow } from "./Group";

test("a native control is labelled by its visible row title", () => {
  render(<GroupContainer><GroupRow title="Review requests" description="Ask before saving." controlId="review">
    <input id="review" type="checkbox" role="switch" />
  </GroupRow></GroupContainer>);
  const control = screen.getByRole("switch", { name: "Review requests" });
  fireEvent.click(screen.getByText("Review requests"));
  expect(control).toBeChecked();
  expect(screen.getByRole("group", { name: "Review requests" })).toHaveAccessibleDescription("Ask before saving.");
});

test("read-only rows keep their label and value without a fake action", () => {
  const { container } = render(<GroupContainer><GroupRow title="Language" value="Deutsch" /></GroupContainer>);
  expect(screen.getByRole("group", { name: "Language" })).toHaveTextContent("Deutsch");
  expect(container.querySelector("button, input, a")).toBeNull();
});

test("a group footer keeps its explanation next to the action it scopes", () => {
  const onClick = vi.fn();
  render(<GroupContainer><GroupFooter description="Only this group is reset.">
    <button type="button" onClick={onClick}>Reset</button>
  </GroupFooter></GroupContainer>);
  expect(screen.getByText("Only this group is reset.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Reset" }));
  expect(onClick).toHaveBeenCalledTimes(1);
});
