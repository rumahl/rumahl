import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { expect, it } from "vitest";
import { RumahlInput, RumahlMultiSelect } from "./RumahlInputs";

it("selects groups independently and submits repeated native form values", () => {
  function Groups() {
    const [value, setValue] = useState<string[]>(["users"]);
    return (
      <form aria-label="Account">
        <RumahlMultiSelect
          label="Groups"
          name="groups"
          value={value}
          onChange={setValue}
          options={[
            { value: "users", label: "Users" },
            { value: "admins", label: "Admins" },
          ]}
        />
      </form>
    );
  }
  render(<Groups />);
  fireEvent.click(screen.getByLabelText("Admins"));
  const form = screen.getByRole("form") as HTMLFormElement;
  expect(new FormData(form).getAll("groups")).toEqual(["users", "admins"]);
  fireEvent.click(screen.getByLabelText("Users"));
  expect(new FormData(form).getAll("groups")).toEqual(["admins"]);
});

it("disables a complete group and associates its feedback", () => {
  render(
    <RumahlMultiSelect
      label="Groups"
      disabled
      status="error"
      description="Select a group"
      value={[]}
      onChange={() => {}}
      options={[{ value: "users", label: "Users" }]}
    />,
  );
  expect(screen.getByRole("checkbox")).toBeDisabled();
  expect(screen.getByRole("group")).toHaveAccessibleDescription(
    "Select a group",
  );
  expect(screen.getByRole("group")).toHaveAttribute("aria-invalid", "true");
});

it("clears the accessible error when validation succeeds", () => {
  const { rerender } = render(
    <RumahlInput aria-label="Hostname" status="error" />,
  );
  expect(screen.getByRole("textbox")).toHaveAttribute("aria-invalid", "true");
  rerender(<RumahlInput aria-label="Hostname" status="success" />);
  expect(screen.getByRole("textbox")).not.toHaveAttribute("aria-invalid");
  expect(screen.getByRole("textbox")).toHaveAttribute("data-status", "success");
});
it("announces a custom error and preserves input focus when it is cleared", () => {
  const { rerender } = render(
    <RumahlInput aria-label="Folder" error="Folder already exists" />,
  );
  const input = screen.getByRole("textbox");
  input.focus();
  expect(input).toHaveAccessibleDescription("Folder already exists");
  expect(screen.getByRole("alert")).toHaveTextContent("Folder already exists");
  rerender(<RumahlInput aria-label="Folder" error="" />);
  expect(screen.getByRole("textbox")).toBe(input);
  expect(input).toHaveFocus();
  expect(input).not.toHaveAttribute("aria-invalid");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
