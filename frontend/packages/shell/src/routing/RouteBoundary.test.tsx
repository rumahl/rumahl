import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { expect, test, vi } from "vitest";
import { RouteBoundary } from "./RouteBoundary";

test("offers recovery without reloading the shell", () => {
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  let fails = true;
  function View() { if (fails) throw Error("fixture"); return <p>Recovered view</p>; }
  try {
    render(<MemoryRouter><RouteBoundary location="/app/test"><View /></RouteBoundary></MemoryRouter>);
    expect(screen.getByRole("alert")).toHaveTextContent("This view is currently unavailable");
    expect(screen.getByRole("link", { name: "Go to apps" })).toHaveAttribute("href", "/apps");
    fails = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(screen.getByText("Recovered view")).toBeInTheDocument();
  } finally { errors.mockRestore(); }
});
