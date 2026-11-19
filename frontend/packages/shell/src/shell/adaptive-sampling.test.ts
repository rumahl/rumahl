import { describe, expect, test } from "vitest";
import {
  SAMPLE_LOW_POWER_MS,
  SAMPLE_STATIC_MS,
  SAMPLE_VIDEO_CALM_MS,
  SAMPLE_VIDEO_DYNAMIC_MS,
  adaptiveSampleInterval
} from "./adaptive-sampling";

const calmVideo = { videoPlaying: true, sceneDelta: 0.01, reducedMotion: false, ultraPerformance: false, hidden: false };

describe("adaptive sampling cadence", () => {
  test("uses a slow read loop for static wallpaper", () => {
    expect(adaptiveSampleInterval({ ...calmVideo, videoPlaying: false })).toBe(SAMPLE_STATIC_MS);
  });
  test("speeds up only while a video scene changes", () => {
    expect(adaptiveSampleInterval(calmVideo)).toBe(SAMPLE_VIDEO_CALM_MS);
    expect(adaptiveSampleInterval({ ...calmVideo, sceneDelta: 0.12 })).toBe(SAMPLE_VIDEO_DYNAMIC_MS);
  });
  test("backs off for reduced motion, ultra performance and hidden tabs", () => {
    expect(adaptiveSampleInterval({ ...calmVideo, reducedMotion: true })).toBe(SAMPLE_LOW_POWER_MS);
    expect(adaptiveSampleInterval({ ...calmVideo, ultraPerformance: true })).toBe(SAMPLE_LOW_POWER_MS);
    expect(adaptiveSampleInterval({ ...calmVideo, hidden: true })).toBe(SAMPLE_LOW_POWER_MS);
  });
});
