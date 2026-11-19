import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { RumahlInput, RumahlMultiSelect, RumahlSwitch, RumahlRange } from "./RumahlInputs";

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

function pointer(input: HTMLElement, type: string, clientX: number) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX });
  Object.defineProperty(event, "pointerId", { value: 1 });
  fireEvent(input, event);
}

it("drags a controlled switch in both directions and only changes once per gesture", () => {
  const changed = vi.fn();
  function Example() {
    const [checked, setChecked] = useState(false);
    return <RumahlSwitch label="Network" checked={checked} onChange={event => {
      changed(event.target.checked);
      setChecked(event.target.checked);
    }} />;
  }
  render(<Example />);
  const input = screen.getByRole("switch");
  pointer(input, "pointerdown", 10);
  pointer(input, "pointermove", 28);
  expect(input).toHaveAttribute("data-dragging", "true");
  pointer(input, "pointerup", 28);
  fireEvent.click(input, { detail: 1 });
  expect(input).toBeChecked();
  expect(changed).toHaveBeenCalledTimes(1);
  pointer(input, "pointerdown", 28);
  pointer(input, "pointermove", 10);
  pointer(input, "pointerup", 10);
  fireEvent.click(input, { detail: 1 });
  expect(input).not.toBeChecked();
  expect(changed).toHaveBeenCalledTimes(2);
});

it("cancels a switch drag without changing its native form value", () => {
  render(<form aria-label="Preferences"><RumahlSwitch label="Network" name="network" defaultChecked /></form>);
  const input = screen.getByRole("switch");
  pointer(input, "pointerdown", 28);
  pointer(input, "pointermove", 10);
  pointer(input, "pointercancel", 10);
  expect(input).toBeChecked();
  expect(input).not.toHaveAttribute("data-dragging");
  expect(new FormData(screen.getByRole("form") as HTMLFormElement).get("network")).toBe("on");
  fireEvent.click(input);
  expect(input).not.toBeChecked();
});

it("does not drag a disabled switch", () => {
  const changed = vi.fn();
  render(<RumahlSwitch label="Network" disabled onChange={changed} />);
  const input = screen.getByRole("switch");
  pointer(input, "pointerdown", 10);
  pointer(input, "pointermove", 28);
  pointer(input, "pointerup", 28);
  expect(input).not.toBeChecked();
  expect(input).not.toHaveAttribute("data-dragging");
  expect(changed).not.toHaveBeenCalled();
});

it("keeps the animated slider aligned with native form values and reset", async () => {
  render(<form aria-label="Audio"><RumahlRange label="Volume" name="volume" min={0} max={100} defaultValue={30} /></form>);
  const slider = screen.getByRole("slider");
  fireEvent.change(slider, { target: { value: "75" } });
  expect(new FormData(screen.getByRole("form") as HTMLFormElement).get("volume")).toBe("75");
  expect(slider.parentElement).toHaveStyle({ "--range-position": "75%" });
  (screen.getByRole("form") as HTMLFormElement).reset();
  await waitFor(() => expect(slider.parentElement).toHaveStyle({ "--range-position": "30%" }));
});
