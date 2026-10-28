import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

const iPhone = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148";

function setViewport(width: number, height: number) {
  Object.defineProperty(window, "innerWidth", { configurable: true, writable: true, value: width });
  Object.defineProperty(window, "innerHeight", { configurable: true, writable: true, value: height });
}

describe("mobile shell", () => {
  let userAgent: PropertyDescriptor | undefined;
  let width: PropertyDescriptor | undefined;
  let height: PropertyDescriptor | undefined;
  beforeEach(() => {
    userAgent = Object.getOwnPropertyDescriptor(window.navigator, "userAgent");
    width = Object.getOwnPropertyDescriptor(window, "innerWidth");
    height = Object.getOwnPropertyDescriptor(window, "innerHeight");
    Object.defineProperty(window.navigator, "userAgent", { configurable: true, value: iPhone });
    localStorage.setItem("rumahl.demo.shell-mode", "desktop");
  });
  afterEach(() => {
    if (userAgent) Object.defineProperty(window.navigator, "userAgent", userAgent);
    else Reflect.deleteProperty(window.navigator, "userAgent");
    if (width) Object.defineProperty(window, "innerWidth", width);
    else Reflect.deleteProperty(window, "innerWidth");
    if (height) Object.defineProperty(window, "innerHeight", height);
    else Reflect.deleteProperty(window, "innerHeight");
  });

  test("starts in the mobile shell, hints in portrait and allows the desktop in landscape", async () => {
    setViewport(390, 844);
    render(<App snapshot={demoSnapshot} initialLocation="/" />);
    const shell = document.querySelector(".shell");
    await waitFor(() => expect(shell).toHaveAttribute("data-shell-mode", "mobile"));
    expect(shell).toHaveAttribute("data-orientation", "portrait");

    fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
    expect(await screen.findByText("Desktop in landscape")).toBeInTheDocument();
    expect(shell).toHaveAttribute("data-shell-mode", "mobile");

    setViewport(844, 390);
    fireEvent(window, new Event("resize"));
    await waitFor(() => expect(shell).toHaveAttribute("data-shell-mode", "desktop"));
    expect(shell).toHaveAttribute("data-orientation", "landscape");
    expect(screen.queryByText("Desktop in landscape")).not.toBeInTheDocument();
  });
});
