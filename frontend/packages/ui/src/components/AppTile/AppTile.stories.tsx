import type { Meta, StoryObj } from "@storybook/react-vite";
import { AppTile } from "@rumahl/ui/components";
import { DocumentIcon } from "@rumahl/ui/icons";

const meta = {
  title: "Components/AppTile",
  component: AppTile,
  args: { title: "Files", id: "files", onClick: () => {} },
  argTypes: { system: { control: "boolean" }, launchable: { control: "boolean" } },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `A launcher icon for an app.

## Rules
- The icon is the app's mark; the label is its name.
- \`system\` marks a protected app; \`launchable\` toggles the launch affordance.
- Part of the **deliberate flat exception** (theme icon gradient), like the
  desktop and launcher.`
      }
    }
  }
} satisfies Meta<typeof AppTile>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <div style={{ display: "flex", gap: 16 }}>
      <AppTile {...args} settingsHref="#files-settings" />
      <AppTile title="Photos" id="photos" onClick={() => {}} settingsHref="#photos-settings" />
      <AppTile title="Settings" id="settings" system onClick={() => {}} />
      <AppTile title="Docs" id="docs" icon={<DocumentIcon />} onClick={() => {}} />
    </div>
  )
};

export const NotLaunchable: Story = {
  render: (args) => (
    <AppTile {...args} title="Terminal" id="terminal" launchable={false} notLaunchableLabel="Not installed" />
  )
};
