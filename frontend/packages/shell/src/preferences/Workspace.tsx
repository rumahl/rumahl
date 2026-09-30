import { createContext, useContext, useEffect, useRef, useState, type PropsWithChildren } from "react";
import type { ShellLiveSource } from "../live-updates";
import { browserProfile } from "./storage";
import type { PreferenceScope } from "./client";
import type { WindowRect, WindowPlacement } from "../shell/desktop/geometry";
export interface AppFolder { id: string; name: string; apps: string[] }
export interface SavedWindow { location: string; rect: WindowRect; placement: WindowPlacement; minimized: boolean }
export interface DesktopArrangement { order: string[]; hidden: string[]; widgets: boolean }
export type LauncherViewId = "grid" | "deck" | "canvas";
export type AppearanceMode = "light" | "dark";
/** Device appearance customization: palette seed, light/dark mode, token overrides. */
export type GlassEngine = "css" | "canvas";
export type GlassBackend = "auto" | "svg" | "webgl" | "css";
export type GlassQuality = "auto" | "high" | "balanced" | "low";
export interface AppearanceSettings {
  autoColor?: boolean;
  wallpaperTint?: boolean;
  wallpaperMotion?: boolean;
  glassReduced?: boolean;
  animations?: boolean;
  performanceMode?: boolean;
  glassEngine?: GlassEngine;
  glassEnabled?: boolean;
  glassBackend?: GlassBackend;
  glassQuality?: GlassQuality;
  seed: string | null;
  mode: AppearanceMode | null;
  tokens: { [key: string]: string };
}
export interface Workspace { version: 1; windows: SavedWindow[]; folders: AppFolder[]; desktop: DesktopArrangement; launcherView: LauncherViewId | null; appearance: AppearanceSettings }
interface Record { ownerId: string; revision: number; user: Workspace | null; device: Workspace | null }
export const emptyAppearance: AppearanceSettings = { seed: null, mode: null, tokens: {} };
export const emptyWorkspace: Workspace = { version: 1, windows: [], folders: [], desktop: { order: [], hidden: [], widgets: true }, launcherView: null, appearance: emptyAppearance };
export function parseWorkspace(value: unknown): Workspace {
  const w = value as Workspace | null;
  if (!w || w.version !== 1 || !Array.isArray(w.windows) || w.windows.length > 32 || !Array.isArray(w.folders) || w.folders.length > 32) throw Error("workspace");
  const desktop = w.desktop ?? { order: [], hidden: [], widgets: true };
  const arrangement = [desktop.order, desktop.hidden];
  if (typeof desktop !== "object" || arrangement.some((list) => !Array.isArray(list) || list.length > 512) ||
      typeof desktop.widgets !== "boolean" ||
      arrangement.some((list) => !list.every((id: unknown) => typeof id === "string" && id.length > 0 && id.length <= 255)) ||
      new Set(desktop.order).size !== desktop.order.length || new Set(desktop.hidden).size !== desktop.hidden.length) throw Error("desktop");
  const launcherView = w.launcherView ?? null;
  if (launcherView !== null && launcherView !== "grid" && launcherView !== "deck" && launcherView !== "canvas") throw Error("launcher");
  const appearance = parseAppearance(w.appearance);
  for (const win of w.windows) {
    if (!win || typeof win.location !== "string" || win.location.length > 2048 || !/^\/(app\/|settings(?:\/|$)|activity$)/.test(win.location.split(/[?#]/)[0]!) || /[\\\r\n]/.test(win.location) || !["floating", "left", "right", "maximized"].includes(win.placement) || typeof win.minimized !== "boolean" || !win.rect || ![win.rect.x,win.rect.y,win.rect.width,win.rect.height].every(v => Number.isFinite(v) && v >= 0 && v <= 32768) || win.rect.width < 1 || win.rect.height < 1) throw Error("window");
  }
  const ids = new Set<string>(), apps = new Set<string>();
  for (const f of w.folders) {
    if (!f || typeof f.id !== "string" || !f.id || f.id.length > 64 || ids.has(f.id) || typeof f.name !== "string" || !f.name.trim() || f.name.length > 128 || /\p{Cc}/u.test(f.name) || !Array.isArray(f.apps) || f.apps.length > 128) throw Error("folder");
    ids.add(f.id);
    for (const id of f.apps) { if (typeof id !== "string" || !/^[a-zA-Z0-9._-]{1,255}$/.test(id) || apps.has(id)) throw Error("app"); apps.add(id); }
  }
  return { ...w, desktop, launcherView, appearance };
}
export function parseAppearance(value: unknown): AppearanceSettings {
  if (!value || typeof value !== "object" || Array.isArray(value)) return emptyAppearance;
  const record = value as { [key: string]: unknown };
  const seed = typeof record.seed === "string" && /^#[0-9a-fA-F]{6}$/.test(record.seed) ? record.seed : null;
  const mode = record.mode === "light" || record.mode === "dark" ? record.mode : null;
  const tokens: { [key: string]: string } = {};
  if (record.tokens && typeof record.tokens === "object" && !Array.isArray(record.tokens)) {
    for (const [key, raw] of Object.entries(record.tokens as { [key: string]: unknown })) {
      if (typeof raw === "string" && raw.length > 0 && raw.length <= 256 && !/[;{}\\<>@]/.test(raw)) tokens[key] = raw;
    }
  }
  return {
    seed, mode, tokens,
    ...(record.autoColor === true ? { autoColor: true } : {}),
    ...(typeof record.wallpaperTint === "boolean" ? { wallpaperTint: record.wallpaperTint } : {}),
    ...(typeof record.wallpaperMotion === "boolean" ? { wallpaperMotion: record.wallpaperMotion } : {}),
    ...(typeof record.glassReduced === "boolean" ? { glassReduced: record.glassReduced } : {}),
    ...(typeof record.animations === "boolean" ? { animations: record.animations } : {}),
    ...(typeof record.performanceMode === "boolean" ? { performanceMode: record.performanceMode } : {}),
    ...(record.glassEngine === "css" || record.glassEngine === "canvas" ? { glassEngine: record.glassEngine } : {}),
    ...(typeof record.glassEnabled === "boolean" ? { glassEnabled: record.glassEnabled } : {}),
    ...(["auto", "svg", "webgl", "css"].includes(record.glassBackend as string) ? { glassBackend: record.glassBackend as GlassBackend } : {}),
    ...(["auto", "high", "balanced", "low"].includes(record.glassQuality as string) ? { glassQuality: record.glassQuality as GlassQuality } : {})
  };
}
interface ContextValue { record: Record | null; effective: Workspace; ready: boolean; busy: boolean; error: string | null; save: (scope: PreferenceScope, value: Workspace | null) => Promise<boolean>; restore: number; requestRestore: () => void }
const Context = createContext<ContextValue | null>(null);
/** Seeds the device-scoped workspace from the server snapshot for first paint. */
function snapshotRecord(initial: string | null | undefined): Record | null {
  if (!initial) return null;
  try {
    return { ownerId: "snapshot", revision: 0, user: null, device: parseWorkspace(JSON.parse(initial)) };
  } catch {
    return null;
  }
}
export function WorkspaceProvider({ live, initial, children }: PropsWithChildren<{ live: ShellLiveSource | undefined; initial?: string | null | undefined }>) {
  const [record,setRecord] = useState<Record | null>(() => snapshotRecord(initial)), [busy,setBusy] = useState(false), [error,setError] = useState<string | null>(null), [restore,setRestore] = useState(0);
  const latest = useRef<Record | null>(null), writer = useRef<ContextValue["save"]>(async()=>false);
  useEffect(() => {
    const controller = new AbortController(), profile = browserProfile(); let writing = false, generation = 0;
    const path = `/api/v1/shell/workspace?device=${profile.id}`;
    function apply(r: Record) { if (controller.signal.aborted || (latest.current?.ownerId === r.ownerId && latest.current.revision > r.revision)) return; latest.current=r; setRecord(r); }
    async function request(update?: {revision:number;scope:PreferenceScope;value:Workspace|null}) {
      const request = live!.request;
      const response = await request(path,{method:update?"PUT":"GET",credentials:"same-origin",cache:"no-store",signal:controller.signal,headers:{"Content-Type":"application/json"},...(update?{body:JSON.stringify(update)}:{})});
      if (!response.ok) throw Error(response.status===409?"conflict":"unavailable");
      const text=await response.text(); if(text.length>140000) throw Error("unavailable");
      const r=JSON.parse(text) as Record;
      if(typeof r.ownerId!=="string" || !Number.isSafeInteger(r.revision) || r.revision<0) throw Error("unavailable");
      if(r.user!==null)parseWorkspace(r.user);if(r.device!==null)parseWorkspace(r.device); return r;
    }
    async function refresh() { if(writing || controller.signal.aborted)return; const gen=generation; try {const r=await request();if(gen===generation){apply(r);setError(previous=>previous==="unavailable"?null:previous);}}catch{if(!controller.signal.aborted)setError("unavailable");} }
    if(live) void refresh(); else { let value=null;try{const text=localStorage.getItem("rumahl.demo.workspace");if(text)value=parseWorkspace(JSON.parse(text));}catch{/* demo storage unavailable */}apply({ownerId:"demo",revision:0,user:value,device:null}); }
    writer.current=async(scope,value)=>{
      if(writing || !latest.current)return false;
      writing=true;generation++;setBusy(true);setError(null);
      try { if(value)parseWorkspace(value);
        if(live)apply(await request({revision:latest.current.revision,scope,value}));
        else {const next={...latest.current,[scope]:value,revision:latest.current.revision+1};apply(next);try{localStorage.setItem("rumahl.demo.workspace",JSON.stringify(next.device??next.user??emptyWorkspace));}catch{/* demo remains usable */}}
        return true;
      } catch(e) {if(!controller.signal.aborted){setError(e instanceof Error&&e.message==="conflict"?"conflict":"unavailable");if(live)try{apply(await request());}catch{/* keep last confirmed workspace */}}return false;
      } finally {writing=false;if(!controller.signal.aborted)setBusy(false);}
    };
    const timer=live?setInterval(()=>void refresh(),5000):undefined;
    const focus=()=>{if(live)void refresh();};window.addEventListener("focus",focus);
    return()=>{controller.abort();clearInterval(timer);window.removeEventListener("focus",focus);writer.current=async()=>false;};
  },[live]);
  return <Context value={{record,effective:record?.device??record?.user??emptyWorkspace,ready:record!==null,busy,error,save:(scope,value)=>writer.current(scope,value),restore,requestRestore:()=>setRestore(v=>v+1)}}>{children}</Context>;
}
export function useWorkspace(){const value=useContext(Context);if(!value)throw Error("Workspace provider missing");return value;}
