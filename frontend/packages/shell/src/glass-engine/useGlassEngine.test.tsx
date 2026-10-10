import { render } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, expect, it, vi } from "vitest";

const mock = vi.hoisted(() => ({ construct: vi.fn(), backend: vi.fn(), destroy: vi.fn() }));
vi.mock("./engine.js", () => ({ GlassEngine: class {
  renderer = "css";
  constructor(options: unknown) { mock.construct(options); }
  mount() { return { update: vi.fn(), destroy: mock.destroy }; }
  setBackend = mock.backend;
  setQuality() {}
  addEventListener() {}
} }));
import { configureGlass, useGlassHost } from "./useGlassEngine";
function Host() {
  const ref = useRef<HTMLDivElement>(null);
  useGlassHost(ref, {});
  return <div ref={ref} />;
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
