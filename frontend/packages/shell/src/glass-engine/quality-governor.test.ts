import { describe, expect, test } from "vitest";
import { QualityGovernor } from "./quality-governor";

const bad = { p95: 34, dropped: 0.5, longTasks: 0, target: 60, sampleCount: 240 };
const good = { p95: 16, dropped: 0.01, longTasks: 0, target: 60, sampleCount: 240 };
const calmLongTask = { ...good, longTasks: 12, p95: 17 };

function governor() {
  return new QualityGovernor({ now: 0, quality: "high", rendererIndex: 0, rendererCount: 2 });
}

describe("glass quality governor", () => {
  test("ignores a single bad window", () => {
    const g = governor();
    expect(g.observe({ ...bad, now: 14_000 })).toBeNull();
    expect(g.quality).toBe("high");
    expect(g.observe({ ...good, now: 18_000 })).toBeNull();
  });

  test("requires sustained stress before one quality step", () => {
    const g = governor();
    expect(g.observe({ ...bad, now: 14_000 })).toBeNull();
    expect(g.observe({ ...bad, now: 18_000 })).toBeNull();
    expect(g.observe({ ...bad, now: 22_000 })).toMatchObject({ type: "quality", quality: "balanced" });
  });

  test("does not downgrade for an isolated long task on an otherwise smooth run", () => {
    const g = governor();
    for (const now of [14_000, 18_000, 22_000, 26_000, 30_000]) {
      expect(g.observe({ ...calmLongTask, now })).toBeNull();
    }
    expect(g.quality).toBe("high");
  });

  test("recovers gradually, not immediately", () => {
    const g = new QualityGovernor({ now: 0, quality: "balanced", rendererIndex: 0, rendererCount: 2 });
    for (const now of [34_000, 38_000, 42_000, 46_000, 50_000]) expect(g.observe({ ...good, now })).toBeNull();
    expect(g.observe({ ...good, now: 54_000 })).toMatchObject({ type: "quality", quality: "high" });
  });

  test("only drops the renderer after the quality ladder is already low", () => {
    const g = new QualityGovernor({ now: 0, quality: "low", rendererIndex: 0, rendererCount: 2 });
    expect(g.observe({ ...bad, now: 34_000 })).toBeNull();
    expect(g.observe({ ...bad, now: 38_000 })).toBeNull();
    expect(g.observe({ ...bad, now: 42_000 })).toMatchObject({ type: "renderer", rendererIndex: 1 });
  });
});
