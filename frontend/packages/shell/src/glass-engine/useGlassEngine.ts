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

function wallpaperSource(): HTMLImageElement | HTMLVideoElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector<HTMLVideoElement>(".wallpaper-video") ?? document.querySelector<HTMLImageElement>(".wallpaper-image");
}

let engine: GlassEngineInstance | null = null;
const config: { backend: GlassBackend; quality: GlassQuality } = { backend: "auto", quality: "auto" };

/** Applies the chosen backend/quality (and master enable) to the shared engine. */
export function configureGlass(next: { enabled?: boolean; backend?: GlassBackend; quality?: GlassQuality }): void {
  if (next.backend) config.backend = next.backend;
  if (next.quality) config.quality = next.quality;
  if (!engine) return;
  engine.setBackend(next.enabled === false ? "css" : config.backend);
  engine.setQuality(config.quality);
}

const handles = new Set<GlassHandle>();
const adjust: Partial<GlassMaterial> = {};

/** Live-tunes refraction/chroma/blur on every mounted glass surface. */
export function setGlassAdjust(next: Partial<GlassMaterial>): void {
  Object.assign(adjust, next);
  for (const handle of handles) handle.update(next);
}

function getEngine(): GlassEngineInstance {
  if (!engine) {
    engine = new Engine({ source: wallpaperSource, quality: config.quality, backend: config.backend });
    const report = () => { if (typeof document !== "undefined") document.documentElement.dataset.glassRenderer = engine!.renderer; };
    engine.addEventListener("qualitychange", report);
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
