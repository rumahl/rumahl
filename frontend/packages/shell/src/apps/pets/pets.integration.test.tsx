import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test } from "vitest";
import { App } from "../../App";
import { demoSnapshot } from "../../demo/snapshot";
import { getPet } from "./pet-store";

beforeEach(() => localStorage.clear());

test("pets is a built-in app and leaves its companion on the desktop", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/" />);

  const tile = await screen.findByRole("link", { name: "Pets" });
  fireEvent.click(tile);

  expect(await screen.findByRole("heading", { name: "Your pet zoo" })).toBeInTheDocument();
  const create = screen.getByRole("button", { name: "Create pet" });
  await waitFor(() => expect(create).toBeEnabled());
  fireEvent.click(create);

  await waitFor(() => expect(getPet("demo")).not.toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "Close Pets" }));

  const companion = await screen.findByRole("button", { name: "Interact with Momo" });
  expect(companion).toHaveAttribute("title", "Momo");
});
