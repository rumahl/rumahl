import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { AppNavLayout } from "./AppNavLayout";
import { GridIcon, SparkIcon } from "../../icons";

it("frames navbar, sidebar and content and forwards the mobile island", () => {
  const change = vi.fn();
  render(<AppNavLayout
    navbar={<div>Top bar</div>}
    sidebar={<nav>Rail</nav>}
    mobileNav={{
      label: "Sections",
      value: "discover",
      onChange: change,
      items: [
        { id: "discover", label: "Discover", icon: <SparkIcon /> },
        { id: "apps", label: "Apps", icon: <GridIcon /> }
      ]
    }}
  >
    <p>Content</p>
  </AppNavLayout>);
  expect(screen.getByText("Top bar")).toBeInTheDocument();
  expect(screen.getByText("Rail")).toBeInTheDocument();
  expect(screen.getByText("Content")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Apps" }));
  expect(change).toHaveBeenCalledWith("apps");
});
