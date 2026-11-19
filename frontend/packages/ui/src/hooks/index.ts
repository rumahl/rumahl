import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject
} from "react";

/** Reactive `matchMedia`. */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== "undefined" && "matchMedia" in window ? window.matchMedia(query).matches : false
  );
  useEffect(() => {
    if (typeof window === "undefined" || !("matchMedia" in window)) return;
    const list = window.matchMedia(query);
    const update = () => setMatches(list.matches);
    update();
    list.addEventListener("change", update);
    return () => list.removeEventListener("change", update);
  }, [query]);
  return matches;
}

/** Boolean state with a stable toggler: `const [on, toggle] = useToggle()`. */
export function useToggle(initial = false): [boolean, (next?: boolean) => void] {
  const [value, setValue] = useState(initial);
  const toggle = useCallback(
    (next?: boolean) => setValue((previous) => (typeof next === "boolean" ? next : !previous)),
    []
  );
  return [value, toggle];
}

/** Calls `handler` on pointer/touch outside `ref` while `enabled`. */
export function useClickOutside<T extends HTMLElement>(
  ref: RefObject<T | null>,
  handler: (event: MouseEvent | TouchEvent) => void,
  enabled = true
): void {
  useEffect(() => {
    if (!enabled) return;
    const listener = (event: MouseEvent | TouchEvent) => {
      const element = ref.current;
      if (!element || element.contains(event.target as Node)) return;
      handler(event);
    };
    document.addEventListener("mousedown", listener);
    document.addEventListener("touchstart", listener);
    return () => {
      document.removeEventListener("mousedown", listener);
      document.removeEventListener("touchstart", listener);
    };
  }, [ref, handler, enabled]);
}

/** Debounced mirror of `value` (updates after `delay` ms of quiet). */
export function useDebounce<T>(value: T, delay = 300): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(id);
  }, [value, delay]);
  return debounced;
}

/** Debounced version of a callback with a stable identity. */
export function useDebouncedCallback<A extends unknown[]>(
  callback: (...args: A) => void,
  delay = 300
): (...args: A) => void {
  const saved = useRef(callback);
  useEffect(() => {
    saved.current = callback;
  });
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  return useCallback(
    (...args: A) => {
      clearTimeout(timer.current);
      timer.current = setTimeout(() => saved.current(...args), delay);
    },
    [delay]
  );
}

/** Adds an event listener to any target while mounted. */
export function useEventListener(
  target: EventTarget | null,
  type: string,
  handler: (event: Event) => void,
  options?: AddEventListenerOptions
): void {
  const saved = useRef(handler);
  useEffect(() => {
    saved.current = handler;
  });
  useEffect(() => {
    if (!target) return;
    const listener = (event: Event) => saved.current(event);
    target.addEventListener(type, listener, options);
    return () => target.removeEventListener(type, listener, options);
  }, [target, type, options]);
}

/** Runs `callback` once after `delay` ms; `null` disables it. */
export function useTimeout(callback: () => void, delay: number | null): void {
  const saved = useRef(callback);
  useEffect(() => {
    saved.current = callback;
  });
  useEffect(() => {
    if (delay === null) return;
    const id = setTimeout(() => saved.current(), delay);
    return () => clearTimeout(id);
  }, [delay]);
}

/** The value from the previous render. */
export function usePrevious<T>(value: T): T | undefined {
  const ref = useRef<T | undefined>(undefined);
  useEffect(() => {
    ref.current = value;
  }, [value]);
  return ref.current;
}

/** A ref plus a callback that focuses it: `const [ref, focus] = useSetFocus()`. */
export function useSetFocus<T extends HTMLElement = HTMLElement>() {
  const ref = useRef<T>(null);
  const setFocus = useCallback(() => ref.current?.focus(), []);
  return [ref, setFocus] as const;
}

/** Calls `handler` when a specific key is pressed (window by default). */
export function useKeyEvent(
  key: string,
  handler: (event: KeyboardEvent) => void,
  options?: { target?: EventTarget | null; when?: boolean }
): void {
  const saved = useRef(handler);
  useEffect(() => {
    saved.current = handler;
  });
  const target = options?.target ?? (typeof window !== "undefined" ? window : null);
  const when = options?.when ?? true;
  useEffect(() => {
    if (!target || !when) return;
    const listener = (event: Event) => {
      if ((event as KeyboardEvent).key === key) saved.current(event as KeyboardEvent);
    };
    target.addEventListener("keydown", listener);
    return () => target.removeEventListener("keydown", listener);
  }, [target, key, when]);
}

/** Tracks pointer hover on an element: `const [ref, hovered] = useHover()`. */
export function useHover<T extends HTMLElement = HTMLElement>() {
  const ref = useRef<T>(null);
  const [hovered, setHovered] = useState(false);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const on = () => setHovered(true);
    const off = () => setHovered(false);
    element.addEventListener("pointerenter", on);
    element.addEventListener("pointerleave", off);
    return () => {
      element.removeEventListener("pointerenter", on);
      element.removeEventListener("pointerleave", off);
    };
  }, []);
  return [ref, hovered] as const;
}

/** Whether an element's content overflows its box. */
export function useIsOverflowing<T extends HTMLElement = HTMLElement>() {
  const ref = useRef<T>(null);
  const [overflowing, setOverflowing] = useState(false);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const check = () =>
      setOverflowing(element.scrollWidth > element.clientWidth || element.scrollHeight > element.clientHeight);
    check();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(check);
    observer?.observe(element);
    return () => observer?.disconnect();
  }, []);
  return [ref, overflowing] as const;
}

/** `false` on the first render, `true` afterwards (for enter animations). */
export function useAfterFirstRender(): boolean {
  const [after, setAfter] = useState(false);
  useEffect(() => setAfter(true), []);
  return after;
}

/**
 * Current time, refreshed on an interval. `null` until mounted (SSR-safe) and
 * paused while the tab is hidden.
 */
export function useClock(intervalMs = 30_000): Date | null {
  const [now, setNow] = useState<Date | null>(null);
  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const stop = () => {
      if (timer) {
        clearInterval(timer);
        timer = undefined;
      }
    };
    const start = () => {
      stop();
      setNow(new Date());
      timer = setInterval(() => {
        if (typeof document === "undefined" || !document.hidden) setNow(new Date());
      }, intervalMs);
    };
    const onVisibility = () => {
      if (typeof document !== "undefined" && document.hidden) stop();
      else start();
    };
    start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [intervalMs]);
  return now;
}
