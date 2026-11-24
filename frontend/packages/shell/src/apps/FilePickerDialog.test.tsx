import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, expect, test, vi } from "vitest";

beforeAll(() => {
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", { configurable: true, value(this: HTMLDialogElement) { this.open = true; } });
  Object.defineProperty(HTMLDialogElement.prototype, "close", { configurable: true, value(this: HTMLDialogElement) { this.open = false; } });
});

const state = vi.hoisted(() => ({
  files: [] as { id: string; name: string; directory: boolean; size: number }[],
  grantError: false
}));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string, values?: Record<string, string>) =>
  values && "title" in values ? `${key}:${values.title}` : key }) }));
const live = {
  request: vi.fn(async () => new Response(JSON.stringify(state.files), { status: 200 }))
};
vi.mock("../shell/ShellContext", () => ({ useShell: () => ({ live }) }));
vi.mock("./client", () => ({
  issueAppGrant: vi.fn(async () => ({})),
}));
const { issueAppGrant } = await import("./client");

import { AppFilePickerDialog } from "./FilePickerDialog";
import type { FilePickerRequest } from "./FilePickerDialog";
import type { InstalledApp } from "./client";

const app: InstalledApp = {
  id: "com.rumahl.notes", installationId: "01990000-0000-7000-8000-000000000001",
  title: "Notes", version: "1.0.0", launchable: true
};

beforeEach(() => {
  state.files = [
    { id: "0123456789abcdef0123456789abcdef", name: "notes.txt", directory: false, size: 42 },
    { id: "0123456789abcdef012345678abcdeff", name: "archive", directory: true, size: 0 }
  ];
  state.grantError = false;
  vi.mocked(issueAppGrant).mockClear();
});

function picker(access: "read" | "write") {
  let settled: { files: { id: string; name: string; size: number }[] } | null = "unset" as never;
  const request: FilePickerRequest = {
    access,
    resolve: (result) => { settled = result; }
  };
  const view = render(<AppFilePickerDialog
    app={app}
    request={request}
    onDone={(result) => { request.resolve(result); }}
  />);
  return { view, settled: () => settled };
}

test("a read consent grants exactly the picked files for the session", async () => {
  const { view, settled } = picker("read");
  expect(await screen.findByRole("dialog", { name: "filesBridge.pickRead" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.grant" }));
  await waitFor(() => expect(settled()).not.toBe("unset"));
  expect(settled()).toEqual({ files: [{ id: "0123456789abcdef0123456789abcdef", name: "notes.txt", size: 42 }] });
  expect(issueAppGrant).toHaveBeenCalledWith(expect.anything(), app, {
    permission: "rumahl.files.read",
    resources: ["0123456789abcdef0123456789abcdef"],
    validity: "session"
  }, expect.anything());
  view.unmount();
});

test("declining resolves as null, and no grant is issued", async () => {
  const { view, settled } = picker("read");
  expect(await screen.findByRole("dialog", { name: "filesBridge.pickRead" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.decline" }));
  await waitFor(() => expect(settled()).not.toBe("unset"));
  expect(settled()).toBeNull();
  expect(issueAppGrant).not.toHaveBeenCalled();
  view.unmount();
});

test("a durable write consent requires the account password", async () => {
  const { view } = picker("write");
  expect(await screen.findByRole("dialog", { name: "filesBridge.pickWrite" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByLabelText("filesBridge.untilRevoked"));
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.grant" }));
  expect(issueAppGrant).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("filesBridge.password"), { target: { value: "hunter2" } });
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.grant" }));
  await waitFor(() => expect(issueAppGrant).toHaveBeenCalledTimes(1));
  const issued = vi.mocked(issueAppGrant).mock.calls[0]?.[2];
  expect(issued).toMatchObject({
    permission: "rumahl.files.write",
    validity: "durable",
    password: "hunter2"
  });
  view.unmount();
});

test("folders are openable and selections do not survive the navigation", async () => {
  const { view, settled } = picker("read");
  expect(await screen.findByRole("checkbox", { name: /notes\.txt/ })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: /notes\.txt/ }));
  fireEvent.click(screen.getByRole("button", { name: /archive\// }));
  await waitFor(() => expect(screen.getByRole("button", { name: "filesBridge.backToFiles" })).toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.backToFiles" }));
  expect(await screen.findByRole("checkbox", { name: /notes\.txt/ })).toBeInTheDocument();
  // The selection was dropped with the navigation; without files, granting is
  // not offered, so the only exit is declining.
  expect(screen.getByRole("button", { name: "filesBridge.grant" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "filesBridge.decline" }));
  await waitFor(() => expect(settled()).not.toBe("unset"));
  expect(settled()).toBeNull();
  expect(issueAppGrant).not.toHaveBeenCalled();
  view.unmount();
});
