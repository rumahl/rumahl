import { fireEvent, render } from "@testing-library/react";
import { expect, test } from "vitest";
import { DEFAULT_PET } from "./pet-model";
import { Pet3D } from "./Pet3D";

test("renders depth layers and tilts toward the pointer", () => {
  const { container } = render(<Pet3D pet={{ ...DEFAULT_PET, render: "3d" }} />);
  const depth = container.querySelectorAll(".pet3d__layer--depth");
  expect(depth.length).toBeGreaterThan(3);
  expect(container.querySelectorAll(".pet3d__layer")).toHaveLength(depth.length);
  expect(container.querySelector(".pet3d__front")).not.toBeNull();
  expect(container.querySelector(".pet3d__front .pet-avatar__eyes")).not.toBeNull();
  expect(container.querySelector(".pet3d__layer--depth .pet-avatar__eyes")).toBeNull();
  expect(container.querySelector(".pet3d__scene")).not.toBeNull();

  const root = container.querySelector(".pet3d") as HTMLElement;
  Object.defineProperty(root, "getBoundingClientRect", {
    configurable: true,
    value: () => ({ x: 0, y: 0, left: 0, top: 0, right: 100, bottom: 100, width: 100, height: 100, toJSON: () => ({}) })
  });
  fireEvent.pointerMove(root, { clientX: 100, clientY: 50, pointerType: "mouse" });
  expect(root.style.getPropertyValue("--pet3d-rotate-y")).toBe("18.00deg");
  expect(root.style.getPropertyValue("--pet3d-gaze-x")).toBe("1.000");
  fireEvent.pointerLeave(root);
  expect(root.style.getPropertyValue("--pet3d-rotate-y")).toBe("0deg");
});

test("soft and deep presets change the extrusion density", () => {
  for (const [depth, expected] of [["soft", 8], ["deep", 16]] as const) {
    const { container, unmount } = render(<Pet3D pet={{ ...DEFAULT_PET, render: "3d", depth }} />);
    expect(container.querySelectorAll(".pet3d__layer--depth")).toHaveLength(expected);
    unmount();
  }
});
