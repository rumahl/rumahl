import type { Meta, StoryObj } from "@storybook/react-vite";
import { AppGrid, AppTile } from "@rumahl/ui/components";

/**
 * # App grid
 *
 * Arranges `AppTile`s for a launcher or desktop. Flat by design (like the
 * tiles). Use `columns` for a fixed grid or omit it for an auto-fill grid.
 */
const APPS = ["Files", "Photos", "Calendar", "Notes", "Mail", "Music", "Maps", "Settings", "Clock"];

const meta = {
  title: "Components/App grid",
  component: AppGrid,
  args: { children: null, columns: 3 },
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: `The launcher grid of \`AppTile\`s.

## Rules
- Fixed \`columns\` and a consistent gap.
- Order by relevance; system apps last.`
      }
    }
  }
} satisfies Meta<typeof AppGrid>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Fixed: Story = {
  render: () => (
    <div style={{ width: 360 }}>
      <AppGrid columns={3}>
        {APPS.map((title) => (
          <AppTile key={title} id={title.toLowerCase()} title={title} onClick={() => {}} />
        ))}
      </AppGrid>
    </div>
  )
};

export const AutoFill: Story = {
  render: () => (
    <div style={{ width: 560 }}>
      <AppGrid>
        {APPS.map((title) => (
          <AppTile key={title} id={title.toLowerCase()} title={title} onClick={() => {}} />
        ))}
      </AppGrid>
    </div>
  )
};
