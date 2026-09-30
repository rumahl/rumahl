import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

test("settings overview keeps existing routes and the active navigation state", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings" />);
  expect(await screen.findByText("Make this desktop your own.")).toBeInTheDocument();
  const navigation = screen.getByRole("navigation", { name: "Settings" });
  const display = within(navigation).getByRole("link", { name: "Personalization" });
  fireEvent.click(display);
  await waitFor(() => expect(display).toHaveAttribute("aria-current", "page"));
  expect(await screen.findByRole("switch", { name: "Transparency effects" })).toBeInTheDocument();
  const workspace = within(navigation).getByRole("link", { name: "Workspace" });
  fireEvent.click(workspace);
  await waitFor(() => expect(workspace).toHaveAttribute("aria-current", "page"));
  expect(await screen.findByRole("button", { name: "Save arrangement" })).toBeInTheDocument();
});
