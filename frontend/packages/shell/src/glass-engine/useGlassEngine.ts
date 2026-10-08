import { useEffect, type RefObject } from "react";
import { GlassEngine } from "./engine.js";
import { svgBackdropSupported } from "./svg-backdrop.js";

export interface GlassMaterial {
  radius?: number;
  bevel?: number;
  refraction?: number;
  chroma?: number;
  blur?: number;
  saturation?: number;
  brightness?: number;
  tint?: string;
}

export type GlassBackend = "auto" | "svg" | "webgl" | "css";
export type GlassQuality = "auto" | "high" | "balanced" | "low";

interface GlassHandle {
  update(patch: Partial<GlassMaterial>): void;
  destroy(): void;
}
interface GlassEngineInstance {
  mount(element: Element, material?: GlassMaterial): GlassHandle;
  setBackend(backend: GlassBackend): void;
  setQuality(quality: GlassQuality): void;
  destroy(): void;
  addEventListener(type: string, listener: (event: Event) => void): void;
  readonly renderer: string;
}
interface GlassEngineOptions {
  source?: () => HTMLImageElement | HTMLVideoElement | null;
  quality?: GlassQuality;
  backend?: GlassBackend;
  targetFps?: number;
}
type GlassEngineCtor = new (options?: GlassEngineOptions) => GlassEngineInstance;

// The vendored engine is plain JS; wrap the constructor with our public types.
const Engine = GlassEngine as unknown as GlassEngineCtor;

/** True when the SVG backdrop-filter path is available in this browser. */
export function svgRefractionSupported(): boolean {
  return typeof window !== "undefined" && svgBackdropSupported();
}

let webglProbe: boolean | null = null;
/** True when this browser can create a WebGL context (probed once). */
export function webglSupported(): boolean {
  if (webglProbe !== null) return webglProbe;
  if (typeof document === "undefined") return false;
  try {
    const canvas = document.createElement("canvas");
    const gl = (canvas.getContext("webgl") ?? canvas.getContext("experimental-webgl")) as WebGLRenderingContext | null;
    webglProbe = !!gl;
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
  } catch {
    webglProbe = false;
  }
  return webglProbe;
}

function wallpaperSource(): HTMLImageElement | HTMLVideoElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector<HTMLVideoElement>(".wallpaper-video") ?? document.querySelector<HTMLImageElement>(".wallpaper-image");
}

let engine: GlassEngineInstance | null = null;
const config: { enabled: boolean; backend: GlassBackend; quality: GlassQuality } = { enabled: true, backend: "auto", quality: "auto" };

/** Applies the chosen backend/quality (and master enable) to the shared engine. */
export function configureGlass(next: { enabled?: boolean; backend?: GlassBackend; quality?: GlassQuality }): void {
  if (next.enabled !== undefined) config.enabled = next.enabled;
  if (next.backend) config.backend = next.backend;
  if (next.quality) config.quality = next.quality;
  if (!engine) return;
  engine.setBackend(config.enabled ? config.backend : "css");
  engine.setQuality(config.quality);
}

const handles = new Set<GlassHandle>();
const adjust: Partial<GlassMaterial> = {};
type GlassMetrics = { p95: number; dropped: number; renderer: string };
const metricListeners = new Set<(metrics: GlassMetrics) => void>();

/** Subscribes to engine performance metrics (for adapt-to-performance hints). */
export function subscribeGlassMetrics(listener: (metrics: GlassMetrics) => void): () => void {
  metricListeners.add(listener);
  return () => { metricListeners.delete(listener); };
}

/** Live-tunes refraction/chroma/blur on every mounted glass surface. */
export function setGlassAdjust(next: Partial<GlassMaterial>): void {
  Object.assign(adjust, next);
  for (const handle of handles) handle.update(next);
}

function getEngine(): GlassEngineInstance {
  if (!engine) {
    engine = new Engine({ source: wallpaperSource, quality: config.quality, backend: config.enabled ? config.backend : "css" });
    const report = () => { if (typeof document !== "undefined") document.documentElement.dataset.glassRenderer = engine!.renderer; };
    engine.addEventListener("qualitychange", report);
    engine.addEventListener("metrics", (event) => {
      const detail = (event as CustomEvent<GlassMetrics>).detail;
      if (detail) for (const listener of metricListeners) listener(detail);
    });
    report();
    // Console diagnostics: `window.__rumahlGlass.setBackend('webgl')`.
    if (typeof window !== "undefined") (window as unknown as { __rumahlGlass?: GlassEngineInstance }).__rumahlGlass = engine;
  }
  return engine;
}

/** Mounts the shared glass engine on a host element (adds `.rumahl-glass-host`). */
export function useGlassHost(ref: RefObject<HTMLElement | null>, material: GlassMaterial): void {
  const key = JSON.stringify(material);
  useEffect(() => {
    const element = ref.current;
    // The engine needs browser media APIs; stay inert in non-browser environments.
    if (!element || typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const handle = getEngine().mount(element, { ...material, ...adjust });
    handles.add(handle);
    return () => { handles.delete(handle); handle.destroy(); };
    // The material is compared by its serialized value.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ref, key]);
}
