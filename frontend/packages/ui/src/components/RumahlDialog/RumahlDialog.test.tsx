import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { RumahlDialog } from "./RumahlDialog";

afterEach(() => {
  Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
});

it("opens modally, cancels on Escape, and restores the trigger focus", () => {
  const show = vi.fn(function (this: HTMLDialogElement) {
    this.open = true;
  });
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value: show,
  });
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value: function (this: HTMLDialogElement) {
      this.open = false;
    },
  });
  const cancel = vi.fn(),
    confirm = vi.fn();
  const trigger = document.createElement("button");
  document.body.append(trigger);
  trigger.focus();
  const props = {
    onClose: cancel,
    onConfirm: confirm,
    title: "Remove folder?",
    description: "This cannot be undone",
    cancelLabel: "Cancel",
    confirmLabel: "Remove",
    destructive: true,
  };
  const { rerender } = render(<RumahlDialog {...props} open />);
  expect(show).toHaveBeenCalledOnce();
  expect(screen.getByRole("alertdialog")).toHaveAccessibleName(
    "Remove folder?",
  );
  expect(screen.getByRole("alertdialog")).toHaveAccessibleDescription(
    "This cannot be undone",
  );
  expect(
    screen.getAllByRole("button", { name: "Cancel" }).at(-1),
  ).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(confirm).toHaveBeenCalledOnce();
  fireEvent(
    screen.getByRole("alertdialog"),
    new Event("cancel", { cancelable: true }),
  );
  expect(cancel).toHaveBeenCalledOnce();
  rerender(<RumahlDialog {...props} open={false} />);
  expect(trigger).toHaveFocus();
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  trigger.remove();
});
