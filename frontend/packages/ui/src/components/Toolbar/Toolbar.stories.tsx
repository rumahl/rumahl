import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton, Toolbar, ToolbarGroup, ToolbarSpacer } from "@rumahl/ui/components";
import { ArrowLeftIcon, SearchIcon, SettingsIcon, SlidersIcon } from "@rumahl/ui/icons";

const meta = {
  title: "Components/Toolbar",
  component: Toolbar,
  args: { children: null },
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: `A floating bar of controls over content (an island).

## Rules
- Group adjacent controls; a **single** control is its own island, not a bar.
- Keep it **floating** (over content), never recessed inside a surface.
- Use \`ToolbarSeparator\` between groups.`
      }
    }
  }
} satisfies Meta<typeof Toolbar>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div style={{ padding: 24 }}>
      <Toolbar>
        <ToolbarGroup>
          <IconButton label="Back">
            <ArrowLeftIcon />
          </IconButton>
          <IconButton label="View options" shape="circle">
            <SlidersIcon />
          </IconButton>
        </ToolbarGroup>
        <ToolbarSpacer />
        <ToolbarGroup>
          <IconButton label="Search">
            <SearchIcon />
          </IconButton>
          <IconButton label="Settings">
            <SettingsIcon />
          </IconButton>
        </ToolbarGroup>
      </Toolbar>
    </div>
  )
};
