import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { SettingsReset } from "./SettingsReset";

test("a reset action pairs the group explanation with a disabled-aware button", () => {
  const onReset = vi.fn();
  const view = render(<SettingsReset label="Reset window space" description="Only this group changes." disabled onReset={onReset} />);
  const button = screen.getByRole("button", { name: "Reset window space" });
  expect(button).toBeDisabled();
  expect(screen.getByText("Only this group changes.")).toBeInTheDocument();
  button.click();
  expect(onReset).not.toHaveBeenCalled();

  view.rerender(<SettingsReset label="Reset window space" description="Only this group changes." disabled={false} onReset={onReset} />);
  fireEvent.click(screen.getByRole("button", { name: "Reset window space" }));
  expect(onReset).toHaveBeenCalledTimes(1);
});
