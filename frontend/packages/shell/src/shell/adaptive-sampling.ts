/** Measurement cadence is deliberately independent from visual transition speed. */
export const SAMPLE_STATIC_MS = 1000;
export const SAMPLE_VIDEO_CALM_MS = 500;
export const SAMPLE_VIDEO_DYNAMIC_MS = 250;
export const SAMPLE_LOW_POWER_MS = 1000;

export interface SamplingSignals {
  videoPlaying: boolean;
  sceneDelta: number;
  reducedMotion: boolean;
  ultraPerformance: boolean;
  hidden: boolean;
}

/**
 * Static wallpaper and low-power modes do not need a fast read loop. A playing
 * video is sampled more often only while its scene is actually changing.
 */
export function adaptiveSampleInterval(signals: SamplingSignals): number {
  if (signals.hidden || signals.reducedMotion || signals.ultraPerformance) return SAMPLE_LOW_POWER_MS;
  if (!signals.videoPlaying) return SAMPLE_STATIC_MS;
  return signals.sceneDelta > 0.035 ? SAMPLE_VIDEO_DYNAMIC_MS : SAMPLE_VIDEO_CALM_MS;
}
