import { beforeEach, describe, expect, test } from "vitest";
import {
  configureToasts,
  dismissToast,
  getToasts,
  showToast,
  toast,
  updateToast
} from "./toasts";

beforeEach(() => {
  dismissToast();
  configureToasts({ position: "top-right", maxVisible: 3 });
});

describe("toast store", () => {
  test("uses Ding-like default lifetimes and keeps errors actionable", () => {
    const success = toast.success("Saved");
    expect(getToasts()[0]).toMatchObject({ id: success, variant: "success", duration: 5000 });
    dismissToast();
    toast.error("Failed");
    expect(getToasts()[0]).toMatchObject({ variant: "error", duration: null });
    dismissToast();
    toast.loading("Working");
    expect(getToasts()[0]).toMatchObject({ variant: "loading", duration: null });
    dismissToast();
    toast.info("Delete?", { action: { label: "Undo" } });
    expect(getToasts()[0]?.duration).toBeNull();
  });

  test("counts repeated identical messages instead of stacking twins", () => {
    toast.info("Same");
    toast.info("Same");
    expect(getToasts()).toHaveLength(1);
    expect(getToasts()[0]?.count).toBe(2);
  });

  test("updates in place with progress and content", () => {
    const id = toast.loading("Uploading", { progress: 0 });
    updateToast(id, { title: "Halfway", message: "Halfway", progress: 0.5 });
    expect(getToasts()[0]).toMatchObject({ title: "Halfway", progress: 0.5, variant: "loading" });
    updateToast(id, { variant: "success", title: "Done", message: "Done", duration: 5000, progress: undefined });
    expect(getToasts()[0]).toMatchObject({ variant: "success", duration: 5000 });
    expect(getToasts()[0]?.progress).toBeUndefined();
  });

  test("promise morphs the loading toast into its result", async () => {
    const result = await toast.promise(Promise.resolve(7), {
      loading: "Saving",
      success: (value) => `Saved ${value}`,
      error: "Could not save"
    });
    expect(result).toBe(7);
    expect(getToasts()[0]).toMatchObject({ title: "Saved 7", message: "Saved 7", variant: "success" });
  });

  test("configures position and clamps the visible stack", () => {
    expect(configureToasts({ position: "bottom-left", maxVisible: 9 }))
      .toEqual({ position: "bottom-left", maxVisible: 5 });
    expect(configureToasts({ position: "middle" as never, maxVisible: 0 }))
      .toEqual({ position: "bottom-left", maxVisible: 1 });
  });

  test("keeps app identity and emergency notifications persistent", () => {
    const id = showToast({
      message: "Evacuate",
      emergency: true,
      source: { appId: "com.example.safety", appTitle: "Safety" }
    });
    expect(getToasts()[0]).toMatchObject({
      id,
      variant: "error",
      duration: null,
      emergency: true,
      source: { appId: "com.example.safety", appTitle: "Safety" }
    });
  });

  test("keeps app descriptions and clears them on update", () => {
    const id = showToast({
      title: "Saved",
      message: "Your changes were written.",
      description: "All files are safe.",
      source: { appId: "com.example.notes", appTitle: "Notes" }
    });
    expect(getToasts()[0]).toMatchObject({ description: "All files are safe." });
    updateToast(id, { description: undefined });
    expect(getToasts()[0]?.description).toBeUndefined();
  });

  test("dismisses one or all toasts", () => {
    const first = showToast({ message: "One", duration: null });
    showToast({ message: "Two", duration: null });
    dismissToast(first);
    expect(getToasts()).toHaveLength(1);
    dismissToast();
    expect(getToasts()).toHaveLength(0);
  });
});
