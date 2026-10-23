import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ContextMenu, Text, useContextMenu, type ContextMenuEntry } from "@rumahl/ui/components";
import { DocumentIcon, ExternalLinkIcon, SettingsIcon } from "@rumahl/ui/icons";

/**
 * # Context menu
 *
 * Right-click (or long-press) menu, opened at the pointer via
 * `useContextMenu`. Closes on outside click, Escape, scroll or resize.
 */
const ITEMS: ContextMenuEntry[] = [
  { id: "open", label: "Open", icon: <DocumentIcon />, shortcut: "⌘O" },
  { id: "open-with", label: "Open in new window", icon: <ExternalLinkIcon /> },
  { id: "rename", label: "Rename", shortcut: "↵" },
  "separator",
  { id: "info", label: "Get info", icon: <SettingsIcon /> },
  { id: "trash", label: "Move to trash", danger: true, disabled: false }
];

const meta = {
  title: "Components/ContextMenu",
  component: ContextMenu,
  args: { open: false, x: 0, y: 0, items: ITEMS, onClose: () => {} },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A right-click menu opened at the pointer.

## Rules
- Actions for the item under the pointer; **one verb per item**.
- Destructive action last with \`danger\`.
- Pair with \`useContextMenu\`; close on outside click, Escape, scroll.`
      }
    }
  }
} satisfies Meta<typeof ContextMenu>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const menu = useContextMenu();
    const [selected, setSelected] = useState<string>();
    const items: ContextMenuEntry[] = [
      { id: "open", label: "Open", icon: <DocumentIcon />, shortcut: "⌘O", onSelect: () => setSelected("Open") },
      { id: "open-with", label: "Open in new window", icon: <ExternalLinkIcon />, onSelect: () => setSelected("Open in new window") },
      { id: "rename", label: "Rename", shortcut: "↵", onSelect: () => setSelected("Rename") },
      "separator",
      { id: "info", label: "Get info", icon: <SettingsIcon />, onSelect: () => setSelected("Get info") },
      { id: "trash", label: "Move to trash", danger: true, onSelect: () => setSelected("Move to trash") }
    ];
    return (
      <div style={{ display: "grid", gap: 14, justifyItems: "center" }}>
        <div
          onContextMenu={menu.onContextMenu}
          style={{
            display: "grid",
            placeItems: "center",
            width: 320,
            height: 180,
            border: "1px dashed var(--control-border)",
            borderRadius: 14
          }}
        >
          <Text tone="muted">Right-click anywhere in this area</Text>
        </div>
        {selected ? (
          <Text size="sm" tone="accent">
            Selected: {selected}
          </Text>
        ) : null}
        <ContextMenu open={menu.open} x={menu.x} y={menu.y} onClose={menu.close} items={items} />
      </div>
    );
  }
};
