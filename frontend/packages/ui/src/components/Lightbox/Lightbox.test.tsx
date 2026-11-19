import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { Lightbox } from "./Lightbox";

beforeEach(() => {
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", { configurable: true, value() { this.open = true; } });
  Object.defineProperty(HTMLDialogElement.prototype, "close", { configurable: true, value() { this.open = false; } });
});
afterEach(() => {
  cleanup();
  Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
});
it("wraps keyboard navigation, resets zoom and handles failed media", () => {
  function Gallery() {
    const [index, setIndex] = useState(0);
    return <Lightbox title="Gallery" images={[{ src: "/one.png", alt: "First image" }, { src: "/two.png", alt: "Second image" }]} index={index} onIndexChange={setIndex} onClose={() => {}} labels={{ close: "Close", previous: "Previous", next: "Next", zoomIn: "Zoom", zoomOut: "Fit", unavailable: "Image unavailable" }} />;
  }
  render(<Gallery />);
  fireEvent.click(screen.getByRole("button", { name: "Zoom" }));
  expect(screen.getByRole("button", { name: "Fit" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.keyDown(screen.getByRole("button", { name: "Fit" }), { key: "ArrowLeft" });
  expect(screen.getByRole("img", { name: "Second image" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Zoom" })).toHaveAttribute("aria-pressed", "false");
  fireEvent.error(screen.getByRole("img", { name: "Second image" }));
  expect(screen.getByRole("status")).toHaveTextContent("Image unavailable");
  expect(screen.getByRole("button", { name: "Zoom" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  expect(screen.getByRole("img", { name: "First image" })).toBeInTheDocument();
});
