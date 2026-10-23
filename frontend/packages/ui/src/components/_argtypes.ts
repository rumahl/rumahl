import type { ArgTypes } from "@storybook/react-vite";

/**
 * Native DOM props every component forwards via `...props`. Declared once so
 * the docs table names them (Storybook otherwise only lists our own props).
 */
export const nativeArgTypes = {
  onClick: { control: false, description: "Native click handler (forwarded).", table: { category: "Events" } },
  onMouseDown: { control: false, description: "Native mousedown handler.", table: { category: "Events" } },
  onMouseUp: { control: false, description: "Native mouseup handler.", table: { category: "Events" } },
  onDoubleClick: { control: false, description: "Native double-click handler.", table: { category: "Events" } },
  onFocus: { control: false, description: "Native focus handler.", table: { category: "Events" } },
  onBlur: { control: false, description: "Native blur handler.", table: { category: "Events" } },
  onKeyDown: { control: false, description: "Native keydown handler.", table: { category: "Events" } },
  onKeyUp: { control: false, description: "Native keyup handler.", table: { category: "Events" } },
  onPointerDown: { control: false, description: "Native pointerdown handler.", table: { category: "Events" } },
  onPointerUp: { control: false, description: "Native pointerup handler.", table: { category: "Events" } },
  "aria-label": { control: "text", description: "Accessible name.", table: { category: "Accessibility" } },
  "aria-pressed": { control: "boolean", description: "Pressed state for toggle buttons.", table: { category: "Accessibility" } },
  "aria-disabled": { control: "boolean", description: "Marks the control disabled for AT.", table: { category: "Accessibility" } },
  title: { control: "text", description: "Native tooltip text.", table: { category: "Accessibility" } },
  disabled: { control: "boolean", description: "Disables the control.", table: { category: "State" } },
  name: { control: "text", description: "Form field name.", table: { category: "Native" } },
  type: { control: "inline-radio", options: ["button", "submit", "reset"], description: "Native button type.", table: { category: "Native" } },
  tabIndex: { control: "number", description: "Tab order.", table: { category: "Native" } },
  id: { control: "text", description: "Element id.", table: { category: "Native" } },
  className: { control: "text", description: "Extra class names.", table: { category: "Native" } },
  style: { control: false, description: "Inline style (CSSProperties).", table: { category: "Native" } }
} satisfies ArgTypes;
