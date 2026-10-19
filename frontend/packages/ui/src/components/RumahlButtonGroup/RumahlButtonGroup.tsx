import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { ControlSize } from "../../size";

export interface GroupOption<T extends string> {
  value: T;
  label: string;
  icon?: ReactNode;
  disabled?: boolean;
}
type Props<T extends string> = {
  label: string;
  options: readonly GroupOption<T>[];
  disabled?: boolean;
  iconOnly?: boolean;
  size?: ControlSize | undefined;
} & (
  | {
      mode: "actions";
      value?: never;
      multiple?: never;
      onChange: (value: T) => void;
    }
  | {
      mode?: "toggle";
      multiple?: false;
      value: T;
      onChange: (value: T) => void;
    }
  | {
      mode?: "toggle";
      multiple: true;
      value: readonly T[];
      onChange: (value: T[]) => void;
    }
);

/** Segmented single selection or independent toggle buttons, with native button semantics. */
export function RumahlButtonGroup<T extends string>(props: Props<T>) {
  const groupRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ id: number; start: number; moved: boolean } | null>(null);
  const suppressClick = useRef(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    const group = groupRef.current;
    if (!group || props.multiple || props.mode === "actions") return;
    const measure = () => {
      const buttons = Array.from(group.querySelectorAll<HTMLButtonElement>("button"));
      const selected = buttons.find(button => button.dataset.value === (preview ?? props.value));
      group.dataset.hasSelection = selected ? "true" : "false";
      // While dragging, the thumb tracks the finger (set in the pointer handler);
      // never snap it to a button until the gesture ends.
      if (dragging) return;
      if (selected) {
        group.style.setProperty("--selection-left", `${selected.offsetLeft}px`);
        group.style.setProperty("--selection-width", `${selected.offsetWidth}px`);
      }
    };
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(group);
    group.querySelectorAll("button").forEach(button => observer?.observe(button));
    return () => observer?.disconnect();
  }, [preview, dragging, props.value, props.multiple, props.mode, props.options]);
  function optionAt(group: HTMLDivElement, x: number, y: number) {
    return Array.from(group.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"))
      .find(button => {
        const rect = button.getBoundingClientRect();
        return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
      });
  }
  function keyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const buttons = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    );
    const index = buttons.indexOf(event.target as HTMLButtonElement);
    if (index < 0 || !buttons.length) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : (index + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) %
            buttons.length;
    buttons[next]?.focus();
  }
  return (
    <div
      ref={groupRef}
      data-selection={props.mode === "actions" ? "actions" : props.multiple ? "multiple" : "single"}
      className={
        props.mode === "actions"
          ? "rumahl-button-group rumahl-button-group--actions"
          : "segmented rumahl-toggle-group"
      }
      role="group"
      aria-label={props.label}
      data-size={props.size}
      onKeyDown={keyboard}
      data-draggable={props.mode !== "actions" && !props.multiple && !props.disabled ? "true" : undefined}
      data-dragging={dragging ? "true" : undefined}
      onPointerDown={event => {
        suppressClick.current = false;
        if (props.disabled || props.multiple || props.mode === "actions" || event.button !== 0 || event.isPrimary === false) return;
        if (!(event.target instanceof Element) || event.target.closest("button")?.matches(":disabled")) return;
        drag.current = { id: event.pointerId, start: event.clientX, moved: false };
      }}
      onPointerMove={event => {
        const current = drag.current;
        if (!current || current.id !== event.pointerId) return;
        current.moved ||= Math.abs(event.clientX - current.start) > 4;
        if (current.moved) {
          const group = event.currentTarget;
          group.setPointerCapture?.(event.pointerId);
          setDragging(true);
          // Track the finger pixel-perfectly: centre the shared thumb on the
          // pointer, clamped to the track, instead of snapping to a button.
          const rect = group.getBoundingClientRect();
          const buttons = Array.from(group.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
          const hovered = optionAt(group, event.clientX, event.clientY);
          const width = hovered?.offsetWidth ?? buttons[0]?.offsetWidth ?? 0;
          const left = Math.max(0, Math.min(event.clientX - rect.left - width / 2, group.clientWidth - width));
          group.dataset.hasSelection = "true";
          group.style.setProperty("--selection-left", `${left}px`);
          group.style.setProperty("--selection-width", `${width}px`);
          setPreview(hovered?.dataset.value ?? null);
        }
      }}
      onPointerUp={event => {
        const current = drag.current;
        if (!current || current.id !== event.pointerId) return;
        drag.current = null;
        setDragging(false);
        setPreview(null);
        if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
        if (!current.moved) return;
        suppressClick.current = true;
        const button = optionAt(event.currentTarget, event.clientX, event.clientY);
        const option = props.options.find(item => item.value === button?.dataset.value);
        if (!props.disabled && !props.multiple && props.mode !== "actions" && option && !option.disabled) {
          button?.focus();
          if (option.value !== props.value) props.onChange(option.value);
        }
      }}
      onPointerCancel={() => { drag.current = null; setDragging(false); setPreview(null); }}
      onLostPointerCapture={() => { drag.current = null; setDragging(false); setPreview(null); }}
      onClickCapture={event => {
        if (suppressClick.current && event.detail !== 0) {
          suppressClick.current = false;
          event.preventDefault();
          event.stopPropagation();
        }
      }}
    >
      {props.options.map((option) => (
        <button
          key={option.value}
          data-value={option.value}
          data-drag-preview={preview === option.value ? "true" : undefined}
          type="button"
          disabled={props.disabled || option.disabled}
          aria-label={props.iconOnly ? option.label : undefined}
          title={props.iconOnly ? option.label : undefined}
          aria-pressed={
            props.mode === "actions"
              ? undefined
              : props.multiple
                ? props.value.includes(option.value)
                : props.value === option.value
          }
          onClick={() => {
            if (props.multiple)
              props.onChange(
                props.value.includes(option.value)
                  ? props.value.filter((value) => value !== option.value)
                  : [...props.value, option.value],
              );
            else props.onChange(option.value);
          }}
        >
          {option.icon ? <span aria-hidden="true">{option.icon}</span> : null}
          {props.iconOnly ? null : option.label}
        </button>
      ))}
    </div>
  );
}
