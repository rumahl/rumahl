import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { LoadingStatus } from "./LoadingStatus";

describe("loading status", () => {
  test("communicates a wait with a spinner instead of a sentence", () => {
    const { container } = render(<LoadingStatus label="Loading files" />);

    // The spinner owns the status role and carries the translated label.
    expect(screen.getByRole("status", { name: "Loading files" })).toBeInTheDocument();
    // Nothing is spelled out: the icon is the message.
    expect(container.textContent).toBe("");
    expect(container.querySelector(".rumahl-spinner__arc")).not.toBeNull();
  });
});
