import type { Meta, StoryObj } from "@storybook/react-vite";
import { RumahlBadge, Table } from "@rumahl/ui/components";
import { nativeArgTypes } from "../_argtypes";

const ROWS = [
  { name: "Files", size: "1.2 GB", status: "ok" },
  { name: "Photos", size: "8.4 GB", status: "ok" },
  { name: "Backups", size: "42.1 GB", status: "warning" },
  { name: "Cache", size: "512 MB", status: "ok" }
];

const meta = {
  title: "Components/Table",
  component: Table,
  argTypes: nativeArgTypes,
  args: {
    getRowKey: (row: Record<string, unknown>) => String(row.name),
    columns: [
      { key: "name", header: "Name" },
      { key: "size", header: "Size", align: "end" },
      { key: "status", header: "Status" }
    ],
    rows: ROWS
  },
  parameters: {
    docs: {
      description: {
        component: `Tabular data with aligned columns.

## Rules
- Align numbers to the **end**, text to the start.
- Keep only the columns you need; the first column identifies the row.
- Prefer hairline row separators over heavy zebra/boxing.`
      }
    }
  }
} satisfies Meta<typeof Table>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Striped: Story = { args: { striped: true } };
export const WithRender: Story = {
  args: {
    columns: [
      { key: "name", header: "Name" },
      { key: "size", header: "Size", align: "end" },
      {
        key: "status",
        header: "Status",
        render: (row) => <RumahlBadge tone={row.status === "ok" ? "success" : "warning"}>{String(row.status)}</RumahlBadge>
      }
    ]
  }
};
