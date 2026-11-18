import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { GroupContainer, GroupRow } from "./Group";

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
