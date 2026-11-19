import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { Button } from "./Button";

test("pending actions retain their label and block duplicate activation", () => {
  const onClick = vi.fn();
  const view = render(<Button onClick={onClick} successText="Saved">Save settings</Button>);
  expect(screen.getByRole("button", { name: "Save settings" })).toBeEnabled();
  view.rerender(<Button loading onClick={onClick} successText="Saved">Save settings</Button>);
  const button = screen.getByRole("button", { name: "Save settings" });
  expect(button).toBeDisabled();
  expect(button).toHaveAttribute("aria-busy", "true");
  button.click();
  expect(onClick).not.toHaveBeenCalled();
  expect(button.querySelector(".btn__pending-label")).toHaveTextContent("Save settings");
});
