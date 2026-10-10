import {
  useLayoutEffect,
  useRef,
  type Dispatch,
  type SetStateAction,
} from "react";

/** Keep theme inheritance, but render above clipping/scrolling window surfaces. */
export function useControlPopover<T extends HTMLElement = HTMLButtonElement>(
  open: boolean,
  setOpen: Dispatch<SetStateAction<boolean>>,
  preferredWidth = 0,
) {
  const triggerRef = useRef<T>(null);
  const popupRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const trigger = triggerRef.current;
    const popup = popupRef.current;
    if (!open || !trigger || !popup) return;
    const position = () => {
      const rect = trigger.getBoundingClientRect();
      const gap = 6;
      // Measure the options instead of imposing a wide minimum on short menus.
      if (!preferredWidth) popup.style.width = "max-content";
      const desired =
        preferredWidth ||
        Math.max(rect.width, 96, popup.getBoundingClientRect().width);
      const width = Math.min(desired, 320, window.innerWidth - 24);
      popup.style.width = `${width}px`;
      const below = Math.max(0, window.innerHeight - rect.bottom - gap - 12);
      const above = Math.max(0, rect.top - gap - 12);
      const height = Math.min(popup.scrollHeight + 2, 420);
      const upwards = below < height && above > below;
      popup.style.maxHeight = `${Math.min(420, upwards ? above : below)}px`;
      popup.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`;
      popup.style.top = `${upwards ? Math.max(12, rect.top - gap - popup.getBoundingClientRect().height) : rect.bottom + gap}px`;
    };
    const onToggle = (event: Event) => {
      if ((event as ToggleEvent).newState === "closed") setOpen(false);
    };
    popup.addEventListener("toggle", onToggle);
    // ResizeObserver and the Popover API are absent in some test/embedded environments.
    const popoverSupported = typeof popup.showPopover === "function";
    if (popoverSupported) popup.showPopover();
    else popup.removeAttribute("popover");
    position();
    const observer =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(position)
        : null;
    observer?.observe(trigger);
    observer?.observe(popup);
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
      popup.removeEventListener("toggle", onToggle);
      try {
        if (
          typeof popup.hidePopover === "function" &&
          popup.matches(":popover-open")
        )
          popup.hidePopover();
      } catch {
        // `:popover-open` is unsupported in some environments.
      }
    };
  }, [open, setOpen, preferredWidth]);

  return { triggerRef, popupRef };
}
