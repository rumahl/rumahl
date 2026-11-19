import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion, useIsPresent } from "motion/react";
import defaultAppIcon from "../apps/default-icons/default-app.svg";
import { useI18n } from "../i18n";

export type ToastVariant = "success" | "warning" | "info" | "normal" | "error" | "loading";
export type ToastPosition = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";

export interface ToastButton {
  label: string;
  onClick?: () => void;
  variant?: "primary" | "default";
}
export interface ToastSource {
  appId: string;
  appTitle: string;
  icon?: string | undefined;
}

export interface ToastOptions {
  title?: string | undefined;
  message: string;
  /** Extra secondary copy below the message, used by app notifications. */
  description?: string | undefined;
  variant?: ToastVariant | undefined;
  /** Milliseconds before auto-dismiss; `null` keeps it persistent. */
  duration?: number | null | undefined;
  /** Up to two action buttons. */
  buttons?: ToastButton[] | undefined;
  /** Provide an id to update/recall the same toast later. */
  id?: string | undefined;
  meta?: string | undefined;
  image?: string | undefined;
  progress?: number | undefined;
  /** Native app identity; the app icon is always shown before the status. */
  source?: ToastSource | undefined;
  /** Urgent notification: persistent, high-contrast and announced as an alert. */
  emergency?: boolean | undefined;
}
export interface Toast extends ToastOptions {
  id: string;
  variant: ToastVariant;
  count: number;
  createdAt: number;
}
export interface ToastConfiguration {
  position: ToastPosition;
  /** How many expanded cards are visible at once; stacking is newest first. */
  maxVisible: number;
}

const POSITIONS: readonly ToastPosition[] = [
  "top-left", "top-center", "top-right", "bottom-left", "bottom-center", "bottom-right"
];
const DEFAULT_CONFIG: ToastConfiguration = { position: "top-right", maxVisible: 3 };

let toasts: readonly Toast[] = [];
let config: ToastConfiguration = DEFAULT_CONFIG;
let animationsOn = true;
let counter = 0;
const listeners = new Set<() => void>();
const emit = () => { for (const listener of listeners) listener(); };

/** Enables/disables toast motion (performance mode / reduce animations). */
export function setToastAnimations(value: boolean): void { animationsOn = value; }

function safeImage(value: unknown): string | undefined {
  return typeof value === "string" && value.length <= 2048 &&
    (value.startsWith("https://") || value.startsWith("data:image/"))
    ? value
    : undefined;
}

function safeMeta(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim().slice(0, 64) : undefined;
}

function safeDescription(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim().slice(0, 2048) : undefined;
}

function safeProgress(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(1, Math.max(0, value))
    : undefined;
}

function normalizeButtons(value: ToastButton[] | undefined): ToastButton[] | undefined {
  if (!value?.length) return undefined;
  return value.slice(0, 2).map((button) => ({
    ...button,
    label: String(button.label ?? "").slice(0, 64)
  }));
}

function defaultDuration(variant: ToastVariant, buttons: ToastButton[] | undefined): number | null {
  // Errors, loading and actionable toasts stay until a deliberate dismissal.
  return variant === "error" || variant === "loading" || !!buttons?.length ? null : 5000;
}

/** Shows a toast and returns its id. */
export function showToast(options: ToastOptions): string {
  const emergency = options.emergency === true;
  const variant = options.variant ?? (emergency ? "error" : "normal");
  const buttons = normalizeButtons(options.buttons);
  const duration = options.duration === undefined
    ? (emergency ? null : defaultDuration(variant, buttons))
    : options.duration;
  const id = options.id ?? `toast-${++counter}`;
  const source = options.source &&
    typeof options.source.appId === "string" && options.source.appId.length > 0 && options.source.appId.length <= 255 &&
    typeof options.source.appTitle === "string" && options.source.appTitle.length > 0 && options.source.appTitle.length <= 200
    ? {
        appId: options.source.appId,
        appTitle: options.source.appTitle,
        ...(safeImage(options.source.icon) ? { icon: safeImage(options.source.icon) } : {})
      }
    : undefined;
  const toast: Toast = {
    ...options,
    id,
    variant,
    duration,
    count: 1,
    createdAt: Date.now(),
    emergency,
    ...(source ? { source } : {}),
    ...(buttons ? { buttons } : {}),
    ...(safeMeta(options.meta) ? { meta: safeMeta(options.meta) } : {}),
    ...(safeDescription(options.description) ? { description: safeDescription(options.description) } : {}),
    ...(safeImage(options.image) ? { image: safeImage(options.image) } : {}),
    ...(safeProgress(options.progress) !== undefined ? { progress: safeProgress(options.progress) } : {})
  };

  if (!options.id) {
    // Ding only collapses a repeat of the live/newest toast.
    const newest = toasts[toasts.length - 1];
    const duplicate = newest &&
      newest.title === toast.title && newest.message === toast.message &&
      newest.description === toast.description && newest.variant === toast.variant
      ? newest
      : undefined;
    if (duplicate) {
      const bumped: Toast = {
        ...duplicate,
        count: duplicate.count + 1,
        createdAt: Date.now(),
        duration: toast.duration,
        ...(buttons ? { buttons } : {})
      };
      toasts = [...toasts.filter((entry) => entry.id !== duplicate.id), bumped];
      emit();
      return duplicate.id;
    }
  }

  toasts = [...toasts.filter((entry) => entry.id !== id), toast];
  emit();
  return id;
}

export function updateToast(id: string, patch: Partial<ToastOptions>): void {
  toasts = toasts.map((entry) => {
    if (entry.id !== id) return entry;
    const next: Toast = {
      ...entry,
      ...patch,
      id,
      variant: patch.variant ?? entry.variant
    };
    // Presence matters: an explicit `undefined` clears an optional field,
    // while leaving the property out keeps the current value.
    if (Object.hasOwn(patch, "buttons")) next.buttons = normalizeButtons(patch.buttons);
    if (Object.hasOwn(patch, "meta")) next.meta = safeMeta(patch.meta);
    if (Object.hasOwn(patch, "description")) next.description = safeDescription(patch.description);
    if (Object.hasOwn(patch, "image")) next.image = safeImage(patch.image);
    if (Object.hasOwn(patch, "progress")) next.progress = safeProgress(patch.progress);
    return next;
  });
  emit();
}

/** Dismisses one toast, or all toasts when called without an id. */
export function dismissToast(id?: string): void {
  toasts = id === undefined ? [] : toasts.filter((entry) => entry.id !== id);
  emit();
}

export function configureToasts(patch: Partial<ToastConfiguration>): ToastConfiguration {
  const position = patch.position && POSITIONS.includes(patch.position) ? patch.position : config.position;
  const maxVisible = typeof patch.maxVisible === "number"
    ? Math.min(5, Math.max(1, Math.round(patch.maxVisible)))
    : config.maxVisible;
  config = { position, maxVisible };
  emit();
  return config;
}

function subscribe(listener: () => void): () => void { listeners.add(listener); return () => { listeners.delete(listener); }; }
function getSnapshot(): readonly Toast[] { return toasts; }
function getConfigSnapshot(): ToastConfiguration { return config; }

/** Current toasts (diagnostics). */
export function getToasts(): readonly Toast[] { return toasts; }
export function useToasts(): readonly Toast[] { return useSyncExternalStore(subscribe, getSnapshot, getSnapshot); }
export function useToastConfig(): ToastConfiguration { return useSyncExternalStore(subscribe, getConfigSnapshot, getConfigSnapshot); }
export interface ToastCallOptions {
  description?: string;
  duration?: number | null;
  id?: string;
  action?: ToastButton;
  secondary?: ToastButton;
  meta?: string;
  image?: string;
  progress?: number;
  source?: ToastSource;
  emergency?: boolean;
}

function call(variant: ToastVariant, title: string, options: ToastCallOptions = {}): string {
  const message = options.description ?? "";
  const buttons: ToastButton[] = [];
  if (options.action) buttons.push({ ...options.action, variant: options.action.variant ?? "primary" });
  if (options.secondary) buttons.push({ ...options.secondary, variant: options.secondary.variant ?? "default" });
  return showToast({
    title,
    message,
    variant,
    ...(options.id ? { id: options.id } : {}),
    ...(options.duration === undefined ? {} : { duration: options.duration }),
    ...(buttons.length ? { buttons } : {}),
    ...(options.meta ? { meta: options.meta } : {}),
    ...(options.image ? { image: options.image } : {}),
    ...(options.progress === undefined ? {} : { progress: options.progress }),
    ...(options.source ? { source: options.source } : {}),
    ...(options.emergency ? { emergency: true } : {})
  });
}

function resolveMessage<T>(value: string | ((input: T) => string), input: T): string {
  return typeof value === "function" ? value(input) : value;
}

/** Ding-compatible convenience API used by the shell, apps and the debug console. */
export const toast = {
  success: (title: string, options?: ToastCallOptions) => call("success", title, options),
  error: (title: string, options?: ToastCallOptions) => call("error", title, options),
  warning: (title: string, options?: ToastCallOptions) => call("warning", title, options),
  info: (title: string, options?: ToastCallOptions) => call("info", title, options),
  loading: (title: string, options?: ToastCallOptions) => call("loading", title, { ...options, duration: null }),
  emergency: (title: string, options?: ToastCallOptions) => call("error", title, { ...options, emergency: true }),
  appNotification: () => {
    const image = `data:image/svg+xml,${encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><rect width="80" height="80" fill="#46a0ff"/><circle cx="40" cy="40" r="22" fill="#eaf6ff"/></svg>'
    )}`;
    return showToast({
      title: "Saved",
      message: "Your changes were written.",
      variant: "success",
      meta: "rumahl Notes",
      image,
      source: { appId: "com.rumahl.notes", appTitle: "rumahl Notes" },
      buttons: [
        { label: "Undo", variant: "primary", onClick: () => showToast({ message: "Undo pressed", variant: "info", duration: 3000 }) }
      ]
    });
  },
  update: updateToast,
  dismiss: dismissToast,
  configure: configureToasts,
  list: getToasts,
  promise: async <T,>(task: Promise<T>, messages: {
    loading: string;
    success: string | ((value: T) => string);
    error: string | ((error: unknown) => string);
  }): Promise<T> => {
    const id = showToast({ title: messages.loading, message: messages.loading, variant: "loading", duration: null });
    try {
      const value = await task;
      const text = resolveMessage(messages.success, value);
      updateToast(id, { title: text, message: text, variant: "success", duration: 5000, progress: undefined });
      return value;
    } catch (error) {
      const text = resolveMessage(messages.error, error);
      updateToast(id, { title: text, message: text, variant: "error", duration: null, progress: undefined });
      throw error;
    }
  }
};

/** Installs the global API so apps, themes and the system can raise toasts. */
export function installToastApi(): void {
  if (typeof window === "undefined") return;
  (window as unknown as { rumahlToast?: unknown }).rumahlToast = {
    ...toast,
    show: showToast
  };
}

function ToastIcon({ toast }: { toast: Toast }) {
  if (toast.source) {
    return <span className={`toast__app-icon${toast.emergency ? " is-emergency" : ""}`}>
      <img src={toast.source.icon ?? defaultAppIcon} alt="" />
      <i className={`toast__app-status toast__app-status--${toast.variant}`} aria-hidden="true" />
    </span>;
  }
  if (toast.image) return <img className="toast__image" src={toast.image} alt="" />;
  if (toast.variant === "loading") return <span className="toast__spinner" aria-hidden="true" />;
  if (toast.variant === "success") {
    return <svg className="toast__glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="9" pathLength={1} />
      <path d="m7.9 12.4 3 3 5.3-6" pathLength={1} />
    </svg>;
  }
  if (toast.variant === "error") {
    return <svg className="toast__glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="9" pathLength={1} />
      <path d="M12 7.5v5.5M12 16.4h.01" pathLength={1} />
    </svg>;
  }
  if (toast.variant === "warning") {
    return <svg className="toast__glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 3.5 21 20H3Z" pathLength={1} />
      <path d="M12 9.5v4.2M12 17h.01" pathLength={1} />
    </svg>;
  }
  return <svg className="toast__glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <circle cx="12" cy="12" r="9" pathLength={1} />
    <path d="M12 10.5V17M12 7.5h.01" pathLength={1} />
  </svg>;
}

function ToastItem({ toast: item, active }: { toast: Toast; active: boolean }) {
  const { t } = useI18n();
  const [paused, setPaused] = useState(false);
  const duration = typeof item.duration === "number" && item.duration > 0 ? item.duration : null;
  const primary = item.title ?? item.message;
  const secondary = item.title && item.message && item.message !== item.title ? item.message : "";
  const closable = item.variant !== "loading" && (item.duration === null || !!item.buttons?.length);
  useEffect(() => {
    if (duration === null || paused || !active) return;
    const timer = setTimeout(() => dismissToast(item.id), duration);
    return () => clearTimeout(timer);
  }, [item.id, duration, paused, active]);
  const footerActions = Boolean(item.source && item.buttons?.length);
  const actions = item.buttons?.length ? <div className="toast__actions">
    {item.buttons.map((button, index) => (
      <button key={index} type="button" className={button.variant === "primary" ? "toast__button toast__button--primary" : "toast__button"}
        onClick={() => { button.onClick?.(); dismissToast(item.id); }}>{button.label}</button>
    ))}
  </div> : null;
  const close = closable
    ? <button type="button" className="toast__close" aria-label="Dismiss" onClick={() => dismissToast(item.id)}>✕</button>
    : null;
  const eyebrow = item.source || item.emergency
    ? <div className="toast__eyebrow">
        {item.emergency ? <span className="toast__emergency-label">{t("notifications.emergencyLabel")}</span> : null}
        {item.source ? <span className="toast__app-name">{item.source.appTitle}</span> : null}
        {item.meta ? <span className="toast__meta">{item.meta}</span> : null}
      </div>
    : null;
  return <div
    className={`toast toast--${item.variant}${item.source ? " toast--app" : ""}${item.source && item.image ? " toast--media" : ""}${item.emergency ? " toast--emergency" : ""}${paused ? " is-paused" : ""}`}
    role={item.emergency ? "alert" : "status"}
    onPointerEnter={() => setPaused(true)}
    onPointerLeave={() => setPaused(false)}
    onFocusCapture={() => setPaused(true)}
    onBlurCapture={() => setPaused(false)}
  >
    <span className="toast__icon" aria-hidden="true">
      <ToastIcon key={item.variant} toast={item} />
    </span>
    <div className="toast__body" key={`${item.variant}|${primary}|${secondary}|${item.description ?? ""}|${item.progress ?? ""}`}>
      <div className="toast__main">
        <div className="toast__copy">
          {eyebrow}
          {item.source && item.image && !item.title ? <span className="toast__media"><img src={item.image} alt="" /></span> : null}
          <div className="toast__head">
            <strong className="toast__title">
              {primary}
              {item.count > 1 ? <span className="toast__count">×{item.count}</span> : null}
            </strong>
            {!item.source && item.meta ? <span className="toast__meta">{item.meta}</span> : null}
          </div>
          {item.source && item.image && item.title ? <span className="toast__media"><img src={item.image} alt="" /></span> : null}
          {secondary ? <span className="toast__message">{secondary}</span> : null}
          {item.description ? <span className="toast__description">{item.description}</span> : null}
          {item.progress !== undefined ? <span className="toast__meter" aria-hidden="true"><i style={{ width: `${Math.round(item.progress * 100)}%` }} /></span> : null}
        </div>
        {!footerActions ? actions : null}
        {!footerActions ? close : null}
      </div>
      {footerActions ? <div className="toast__footer">
        {actions}
        {close}
      </div> : null}
    </div>
    {active && duration !== null && item.buttons?.length ? <span className="toast__progress" style={{ "--toast-duration": `${duration}ms` } as CSSProperties} aria-hidden="true" /> : null}
  </div>;
}

function ToastEntry({ item, extra, animationsOn: motionOn }: { item: Toast; extra: number; animationsOn: boolean }) {
  const card = useRef<HTMLDivElement>(null);
  const isPresent = useIsPresent();
  const [size, setSize] = useState({ width: 44, height: 44 });
  const [opened, setOpened] = useState(false);
  const [settled, setSettled] = useState(false);
  useEffect(() => {
    if (!motionOn) { setOpened(true); return; }
    const timer = setTimeout(() => setOpened(true), 450);
    return () => clearTimeout(timer);
  }, [motionOn]);
  useLayoutEffect(() => {
    const element = card.current;
    if (!element) return;
    const measure = () => setSize({
      width: Math.max(44, element.offsetWidth),
      height: Math.max(44, element.offsetHeight)
    });
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(element);
    window.addEventListener("resize", measure);
    return () => { observer?.disconnect(); window.removeEventListener("resize", measure); };
  }, []);
  useEffect(() => {
    if (!motionOn) { setSettled(true); return; }
    const timer = setTimeout(() => setSettled(true), 1400);
    return () => clearTimeout(timer);
  }, [motionOn]);
  const spring = { type: "spring" as const, stiffness: 340, damping: 27, mass: 0.9 };
  return <motion.div
    className={`toast-entry${opened ? " is-open" : ""}${settled ? " is-settled" : ""}${!isPresent ? " is-leaving" : ""}`}
    style={{ transformPerspective: 900 }}
    layout={motionOn ? "position" : false}
    drag={motionOn ? "x" : false}
    dragConstraints={{ left: 0, right: 0 }}
    dragElastic={0.2}
    whileDrag={{ scale: 0.985, opacity: 0.92 }}
    onDragEnd={(_, info) => {
      if (Math.abs(info.offset.x) > 90 || Math.abs(info.velocity.x) > 620) dismissToast(item.id);
    }}
    {...(motionOn ? {
      initial: { opacity: 0, y: -70, scale: 0.5, width: 44, height: 44 },
      exit: {
        width: 44,
        height: 44,
        opacity: 0,
        y: -8,
        scale: 0.92,
        transition: {
          width: { duration: 0.34, ease: [0.2, 0.8, 0.2, 1] as const },
          height: { duration: 0.34, ease: [0.2, 0.8, 0.2, 1] as const },
          opacity: { delay: 0.34, duration: 0.22, ease: "easeOut" as const },
          y: { delay: 0.34, duration: 0.22, ease: "easeOut" as const },
          scale: { delay: 0.34, duration: 0.22, ease: "easeOut" as const }
        }
      }
    } : { initial: false })}
    animate={{
      opacity: 1,
      y: 0,
      x: 0,
      scale: 1,
      width: opened ? size.width : 44,
      height: opened ? size.height : 44
    }}
    transition={motionOn ? spring : { duration: 0 }}
  >
    {extra > 0 ? <div className="toast-stack__ghosts" aria-hidden="true">
      <span key={extra} className="toast-stack__count">+{extra}</span>
    </div> : null}
    <div ref={card} className="toast-entry__card"><ToastItem toast={item} active={opened && isPresent} /></div>
  </motion.div>;
}

/** Stackable toast surface. The newest card is nearest the screen corner. */
export function ToastStack() {
  const list = useToasts();
  const configuration = useToastConfig();
  const [host, setHost] = useState<HTMLElement | null>(null);
  useEffect(() => { setHost(document.body); }, []);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && getToasts().length) {
        dismissToast();
        return;
      }
      if (event.altKey && event.key.toLowerCase() === "t") {
        const close = document.querySelector<HTMLElement>(".toast__close");
        close?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  if (!host || !list.length) return null;
  const visible = list.slice(-configuration.maxVisible).reverse();
  const extra = list.length - visible.length;
  return createPortal(<div className="toast-stack" data-position={configuration.position} aria-label="Notifications">
    <AnimatePresence initial={false} mode="popLayout">
      {visible.map((item, index) => (
        <ToastEntry key={item.id} item={item} extra={index === visible.length - 1 ? extra : 0} animationsOn={animationsOn} />
      ))}
    </AnimatePresence>
  </div>, host);
}

// Keep the console API available even if a host effect is interrupted.
if (typeof window !== "undefined") installToastApi();

if (typeof window !== "undefined") {
  const target = window as unknown as { __rumahlDebug?: Record<string, unknown> };
  target.__rumahlDebug = {
    ...(target.__rumahlDebug ?? {}),
    toast: (message?: string) => toast.info(message ?? `Notification ${toasts.length + 1}`),
    toastCount: () => toasts.length,
    demo: () => {
      toast.success("App installed.", { description: "The package is ready to launch." });
      toast.warning("Low disk space.", { description: "Free up storage soon.", action: { label: "Manage", variant: "primary" } });
      toast.info("This one stays until you close it.", { duration: null });
    },
    appNotification: () => toast.appNotification(),
    emergency: () => toast.emergency("Emergency test", {
      description: "This emergency notification stays until dismissed.",
      source: { appId: "com.rumahl.debug", appTitle: "rumahl Debug" }
    })
  };
}
