import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { RumahlProgress, RumahlSpinner } from "./RumahlProgress";

it("exposes a bounded percentage and keeps an empty track", () => {
  const { rerender } = render(<RumahlProgress label="Upload" value={0} />);
  const track = screen.getByRole("progressbar", { name: "Upload" });
  expect(track).toHaveAttribute("aria-valuenow", "0");
  rerender(<RumahlProgress label="Upload" value={150} />);
  expect(track).toHaveAttribute("aria-valuenow", "100");
  rerender(<RumahlProgress label="Upload" />);
  expect(track).not.toHaveAttribute("aria-valuenow");
  expect(track).toHaveAttribute("data-indeterminate", "true");
});
it("labels the spinner without exposing the decorative arc", () => {
  render(<RumahlSpinner label="Loading files" />);
  expect(screen.getByRole("status", { name: "Loading files" })).toBeInTheDocument();
});
