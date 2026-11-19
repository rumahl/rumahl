/** Ordered quality ladder. Recovery always moves one rung at a time. */
export const QUALITY_LEVELS = Object.freeze(["high", "balanced", "low"]);

const DEFAULTS = Object.freeze({
  warmupMs: 10_000,
  evaluationMs: 4_000,
  qualityDownWindows: 3,
  qualityUpWindows: 6,
  rendererDownWindows: 3,
  rendererUpWindows: 10,
  qualityDownDwellMs: 12_000,
  qualityUpDwellMs: 30_000,
  rendererDownDwellMs: 30_000,
  rendererUpDwellMs: 45_000
});

/**
 * Debounced quality state machine for the glass engine.
 *
 * Down and up use different thresholds, changes require several consecutive
 * evaluation windows, and every transition has a minimum dwell time. Long
 * tasks alone never trigger a downgrade: isolated main-thread work (including
 * the OS itself) must also produce sustained frame-cadence stress.
 */
export class QualityGovernor {
  constructor({ quality = "high", rendererIndex = 0, rendererCount = 1, now = 0, timings = {} } = {}) {
    this.timings = { ...DEFAULTS, ...timings };
    this.quality = quality;
    this.rendererIndex = rendererIndex;
    this.rendererCount = Math.max(1, rendererCount);
    this.lastQualityChange = now;
    this.lastRendererChange = now;
    this.warmupUntil = now + this.timings.warmupMs;
    this.resetCounters();
  }

  resetCounters() {
    this.qualityDown = 0;
    this.qualityUp = 0;
    this.rendererDown = 0;
    this.rendererUp = 0;
  }

  setQuality(quality, now) {
    if (QUALITY_LEVELS.includes(quality)) {
      this.quality = quality;
      this.lastQualityChange = now;
      this.resetCounters();
    }
  }

  setRenderer(index, now) {
    this.rendererIndex = Math.max(0, Math.min(this.rendererCount - 1, index));
    this.lastRendererChange = now;
    this.resetCounters();
  }

  /** Returns a single transition or null. One change per evaluation window. */
  observe({ now, p95, dropped = 0, longTasks = 0, target = 60, sampleCount = 0 }) {
    if (sampleCount < 30 || now < this.warmupUntil || !Number.isFinite(p95) || p95 <= 0) {
      this.resetCounters();
      return null;
    }
    const t = this.timings;
    const budget = 1000 / Math.max(1, target);
    // Deliberately wide hysteresis: a bad second must not instantly disable
    // WebGL, while recovery needs much better sustained cadence.
    const stress = p95 > budget * 1.8 ||
      (dropped > 0.40 && p95 > budget * 1.30) ||
      (longTasks >= 4 && p95 > budget * 1.45);
    const stable = p95 < budget * 1.12 && dropped < 0.06 && longTasks <= 1;

    if (stress) {
      this.qualityDown += 1;
      this.rendererDown += 1;
      this.qualityUp = 0;
      this.rendererUp = 0;
    } else if (stable) {
      this.qualityUp += 1;
      this.rendererUp += 1;
      this.qualityDown = 0;
      this.rendererDown = 0;
    } else {
      this.resetCounters();
      return null;
    }

    if (stress) {
      if (this.quality !== "low" && this.qualityDown >= t.qualityDownWindows && now - this.lastQualityChange >= t.qualityDownDwellMs) {
        const index = QUALITY_LEVELS.indexOf(this.quality);
        const quality = QUALITY_LEVELS[Math.min(QUALITY_LEVELS.length - 1, index + 1)];
        this.quality = quality;
        this.lastQualityChange = now;
        this.resetCounters();
        return { type: "quality", quality, reason: "sustained-stress" };
      }
      if (this.quality === "low" && this.rendererIndex < this.rendererCount - 1 &&
          now - this.lastQualityChange >= t.qualityDownDwellMs &&
          now - this.lastRendererChange >= t.rendererDownDwellMs &&
          this.rendererDown >= t.rendererDownWindows) {
        this.rendererIndex += 1;
        this.lastRendererChange = now;
        this.resetCounters();
        return { type: "renderer", rendererIndex: this.rendererIndex, reason: "sustained-stress" };
      }
      return null;
    }

    // Recovery is the slow path: quality first, renderer only after the highest
    // quality has itself been stable for a long window.
    if (this.rendererIndex > 0 && this.quality === "high" &&
        this.rendererUp >= t.rendererUpWindows &&
        now - this.lastRendererChange >= t.rendererUpDwellMs) {
      this.rendererIndex -= 1;
      this.lastRendererChange = now;
      this.resetCounters();
      return { type: "renderer", rendererIndex: this.rendererIndex, reason: "stable-recovery" };
    }
    if (this.quality !== "high" && this.qualityUp >= t.qualityUpWindows && now - this.lastQualityChange >= t.qualityUpDwellMs) {
      const index = QUALITY_LEVELS.indexOf(this.quality);
      const quality = QUALITY_LEVELS[Math.max(0, index - 1)];
      this.quality = quality;
      this.lastQualityChange = now;
      this.resetCounters();
      return { type: "quality", quality, reason: "stable-recovery" };
    }
    return null;
  }
}
