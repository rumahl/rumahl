import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { RumahlAvatar, RumahlAvatarChange, RumahlPassword } from "./RumahlProfile";

test("shows initials when an avatar fails to load", () => {
  const { container } = render(<RumahlAvatar name="Alex Morgan" src="/missing.jpg" />);
  fireEvent.error(container.querySelector("img")!);
  expect(screen.getByText("AM")).toBeInTheDocument();
});
test("reveals the password without changing it and conceals it on exit", () => {
  render(<RumahlPassword label="Password" defaultValue="test secret" />);
  const input = screen.getByLabelText("Password");
  expect(input).toHaveAttribute("type", "password");
  fireEvent.click(screen.getByRole("button", { name: "Show password" }));
  expect(input).toHaveAttribute("type", "text");
  expect(input).toHaveValue("test secret");
  fireEvent.blur(input, { relatedTarget: document.body });
  expect(input).toHaveAttribute("type", "password");
});
test("rejects unsuitable avatar files without publishing a change", () => {
  const change = vi.fn();
  render(<RumahlAvatarChange name="Alex" onChange={change} />);
  const input = screen.getByLabelText("Change picture", { selector: "input" });
  fireEvent.change(input, { target: { files: [new File(["text"], "notes.txt", { type: "text/plain" })] } });
  expect(screen.getByRole("alert")).toHaveTextContent("5 MB");
  expect(change).not.toHaveBeenCalled();
});
