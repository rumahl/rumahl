import { render } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, expect, it, vi } from "vitest";

const mock = vi.hoisted(() => ({ construct: vi.fn(), backend: vi.fn(), destroy: vi.fn(), mount: vi.fn(), update: vi.fn() }));
vi.mock("./engine.js", () => ({ GlassEngine: class {
  renderer = "css";
  constructor(options: unknown) { mock.construct(options); }
  mount(...args: unknown[]) { mock.mount(...args); return { update: mock.update, destroy: mock.destroy }; }
  setBackend = mock.backend;
  setQuality() {}
  addEventListener() {}
} }));
import { configureGlass, useGlassElementHost, useGlassHost } from "./useGlassEngine";
function Host() {
  const ref = useRef<HTMLDivElement>(null);
  useGlassHost(ref, {});
  return <div ref={ref} />;
}
function ElementHost({ enabled, backend }: { enabled: boolean; backend: "svg" | "webgl" }) {
  const [element, setElement] = useState<HTMLDivElement | null>(null);
  useGlassElementHost(element, {}, enabled, backend);
  return <div ref={setElement} />;
}
afterEach(() => vi.unstubAllGlobals());
it("retains disabled configuration before mounting and across partial updates", () => {
  vi.stubGlobal("matchMedia", vi.fn());
  configureGlass({ enabled: false, backend: "svg" });
  const view = render(<Host />);
  expect(mock.construct).toHaveBeenCalledWith(expect.objectContaining({ backend: "css" }));
  configureGlass({ quality: "low" });
  expect(mock.backend).toHaveBeenLastCalledWith("css");
  configureGlass({ enabled: true });
  expect(mock.backend).toHaveBeenLastCalledWith("svg");
  view.unmount();
  expect(mock.destroy).toHaveBeenCalledOnce();
});
it("mounts a runtime host with its own SVG backend and releases it when disabled", () => {
  vi.stubGlobal("matchMedia", vi.fn());
  vi.clearAllMocks();
  const view = render(<ElementHost enabled backend="svg" />);
  expect(mock.mount).toHaveBeenCalledWith(expect.any(HTMLDivElement), {}, "svg");
  view.rerender(<ElementHost enabled={false} backend="svg" />);
  expect(mock.destroy).toHaveBeenCalled();
});
