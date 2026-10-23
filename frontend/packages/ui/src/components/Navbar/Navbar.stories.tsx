import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton, Navbar, NavbarItem, RumahlMark } from "@rumahl/ui/components";
import { HomeIcon, SearchIcon, SettingsIcon } from "@rumahl/ui/icons";

const meta = {
  title: "Components/Navbar",
  component: Navbar,
  args: { children: null },
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: `A horizontal navigation bar for a few top-level destinations.

## Rules
- Keep it to one line; short, parallel labels.
- **Don't mix navigation and actions** — actions belong in a Toolbar.`
      }
    }
  }
} satisfies Meta<typeof Navbar>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [page, setPage] = useState("home");
    return (
      <div style={{ padding: 24 }}>
        <Navbar
          brand={<RumahlMark style={{ width: 16, height: 28 }} />}
          actions={
            <>
              <IconButton label="Search">
                <SearchIcon />
              </IconButton>
              <IconButton label="Settings">
                <SettingsIcon />
              </IconButton>
            </>
          }
        >
          {[
            { id: "home", label: "Home", icon: <HomeIcon /> },
            { id: "apps", label: "Apps", icon: <SearchIcon /> },
            { id: "activity", label: "Activity", icon: <SettingsIcon /> }
          ].map((item) => (
            <NavbarItem
              key={item.id}
              as="button"
              type="button"
              icon={item.icon}
              current={page === item.id}
              onClick={() => setPage(item.id)}
            >
              {item.label}
            </NavbarItem>
          ))}
        </Navbar>
      </div>
    );
  }
};
