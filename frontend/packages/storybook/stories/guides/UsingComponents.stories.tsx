import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  RumahlBadge,
  RumahlButtonGroup,
  RumahlInput,
  RumahlNotice,
  RumahlProgress,
  RumahlSearch,
  RumahlSelect,
  RumahlStepper,
  RumahlSwitch,
  RumahlTabs
} from "@rumahl/ui/components";
import { Button } from "@rumahl/ui/components";

/**
 * # Using components
 *
 * ## How to work with them
 * 1. **Import** from `@rumahl/ui/components` (barrel) or a subpath
 *    `@rumahl/ui/components/Button`.
 * 2. **Wrap** your app in `ThemeProvider` (and `I18nProvider`) — see
 *    **Getting started**. Import the style layers once.
 * 3. **Set props**: every control takes native element props (`className`,
 *    `style`, `aria-*`, `value`, `onChange` …) plus its own. Controls share a
 *    small vocabulary: `variant`, `size`, `tone`, and the element states
 *    `floating` and `glass={false}` (see **Overrides**).
 * 4. **Restyle** with tokens — never hard-code colours; override tokens in a
 *    theme (see **Custom Themes**).
 *
 * ## What each control offers
 * | Control | Use it for | Key props / states |
 * |---|---|---|
 * | `Button` / `ButtonLink` | actions / navigation | `variant` (primary, secondary, ghost, danger), `size`, `floating`, `glass` |
 * | `RumahlInput` / `RumahlTextArea` | text entry | `status`, `error`, `floating`, `glass` |
 * | `RumahlInputGroup` | field with prefix/suffix | `prefix`, `suffix` |
 * | `RumahlCheckbox` / `RumahlRadio` / `RumahlSwitch` / `RumahlRange` / `RumahlFileInput` | choices | `label`, `checked`/`value` (controlled) |
 * | `RumahlMultiSelect` / `RumahlFieldGroup` | grouped settings | `label`, `options`, `description` |
 * | `RumahlSelect` | dropdown | `label`, `options`, `value` |
 * | `RumahlButtonGroup` | segmented toggle (single/multiple, drag) | `options`, `mode`, `multiple` |
 * | `RumahlTabs` | tabbed sections | `tabs`, `value` |
 * | `RumahlBadge` / `RumahlNotice` / `RumahlProgress` | status / callouts / progress | `tone`, `label`, `value` |
 * | `RumahlDialog` | confirm / destructive alert | `open`, `onConfirm`, `destructive` |
 * | `RumahlCalendar` / `RumahlDatePicker` | dates | `value`, `min`/`max` |
 * | `RumahlAvatar` / `RumahlProfile*` / `RumahlPassword` | accounts | `name`, `size`, `status` |
 * | `RumahlSearch` / `RumahlStepper` | OS controls | `label`, `value` |
 * | `RumahlColorPicker` | colours | `value`, `presets` |
 * | `RumahlMark` / `RumahlPageHeading` / `RumahlEmptyState` | brand / page framing | `title`, `help`, `action` |
 *
 * Every component's own page has an autodocs tab with its full prop table,
 * usage, accessibility notes and the tokens it uses.
 */
const meta = {
  title: "Guides/Using components",
  tags: ["autodocs"],
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: "The control vocabulary and how to start with each one."
      }
    }
  }
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Catalog: Story = {
  render: () => {
    const [value, setValue] = useState("auto");
    const [tab, setTab] = useState("general");
    const [count, setCount] = useState(3);
    const [query, setQuery] = useState("");
    const [on, setOn] = useState(true);
    return (
      <div style={{ display: "grid", gap: 16, width: 420 }}>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <Button variant="primary">Primary</Button>
          <Button>Secondary</Button>
          <Button variant="danger" glass={false}>Danger flat</Button>
        </div>
        <RumahlInput placeholder="Input" />
        <RumahlSelect
          label="Select"
          value={value}
          onChange={setValue}
          options={[{ value: "auto", label: "Auto" }, { value: "manual", label: "Manual" }]}
        />
        <RumahlSwitch label="Switch" checked={on} onChange={(event) => setOn(event.target.checked)} />
        <RumahlSearch label="Search" clearLabel="Clear" value={query} onChange={setQuery} />
        <RumahlStepper label="Count" decreaseLabel="Less" increaseLabel="More" value={count} onChange={setCount} />
        <RumahlButtonGroup
          label="Layout"
          value="list"
          options={[{ value: "list", label: "List" }, { value: "grid", label: "Grid" }]}
          onChange={() => {}}
        />
        <RumahlTabs
          label="Sections"
          value={tab}
          onChange={setTab}
          tabs={[
            { id: "general", label: "General", content: <p style={{ margin: 0 }}>General settings.</p> },
            { id: "network", label: "Network", content: <p style={{ margin: 0 }}>Network settings.</p> }
          ]}
        />
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <RumahlBadge tone="success">Success</RumahlBadge>
          <RumahlBadge tone="warning">Warning</RumahlBadge>
        </div>
        <RumahlNotice title="Notice" tone="info">Callouts share the control material.</RumahlNotice>
        <RumahlProgress label="Progress" value={64} />
      </div>
    );
  }
};
