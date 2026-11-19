import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { beforeAll, beforeEach, afterEach, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

const script = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "../../../../../platform-web/src/login-appearance.js"), "utf8");
const picker = '<section class="login-profiles"><a class="account" href="/login?next=%2Fapp%2Fnotes&user=alice">Alice</a><a class="login-action" href="/login?next=%2Fapp%2Fnotes&user=bob">Bob</a></section>';
const form = (user: string) => `<section class="login-profiles"><h1>${user}</h1><form method="post" action="/login"><input name="username" type="hidden" value="${user}"><input name="next" type="hidden" value="/app/notes"><input name="password" type="password" autofocus><button type="submit">Sign in</button></form><a class="login-action" href="/login?next=%2Fapp%2Fnotes">Switch user</a></section>`;
const response = (html: string) => ({ ok: true, headers: new Headers({ "content-type": "text/html" }), text: async () => html });
const click = (selector: string) => document.querySelector(selector)!.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, button: 0 }));

beforeAll(() => { window.eval(script); });
beforeEach(() => {
  history.replaceState(null, "", "/login?next=%2Fapp%2Fnotes");
  document.body.innerHTML = `<header id="clock">Clock</header>${picker}`;
});
afterEach(() => { document.body.innerHTML = ""; vi.unstubAllGlobals(); });

it("switches views without replacing the clock, clears passwords and preserves native POST", async () => {
  const fetch = vi.fn().mockResolvedValueOnce(response(form("alice"))).mockResolvedValueOnce(response(picker));
  vi.stubGlobal("fetch", fetch);
  const clock = document.getElementById("clock");
  expect(click(".account")).toBe(false);
  await waitFor(() => expect(document.querySelector('input[name="username"]')).toHaveValue("alice"));
  expect(document.getElementById("clock")).toBe(clock);
  expect(location.search).toContain("user=alice");
  const password = document.querySelector<HTMLInputElement>('input[type="password"]')!;
  expect(document.activeElement).toBe(password);
  password.value = "never-retain-this";
  expect(document.querySelector('input[name="next"]')).toHaveValue("/app/notes");
  const submit = new Event("submit", { bubbles: true, cancelable: true });
  document.querySelector("form")!.dispatchEvent(submit);
  expect(submit.defaultPrevented).toBe(false);
  click(".login-action");
  await waitFor(() => expect(document.querySelector(".account")).not.toBeNull());
  expect(password.value).toBe("");
  expect(document.activeElement).toBe(document.querySelector(".account"));
  expect(JSON.stringify(history.state)).not.toContain("never-retain-this");
  expect(fetch.mock.calls[0]![1]).toMatchObject({ credentials: "same-origin", cache: "no-store", redirect: "error" });
});

it("ignores stale view responses and handles browser history without pushing a new entry", async () => {
  let finishFirst!: (value: ReturnType<typeof response>) => void;
  const fetch = vi.fn()
    .mockImplementationOnce(() => new Promise(resolve => { finishFirst = resolve; }))
    .mockResolvedValueOnce(response(form("bob")))
    .mockResolvedValueOnce(response(picker));
  vi.stubGlobal("fetch", fetch);
  click(".account");
  click(".login-action");
  await waitFor(() => expect(document.querySelector('input[name="username"]')).toHaveValue("bob"));
  expect(fetch.mock.calls[0]![1].signal.aborted).toBe(true);
  finishFirst(response(form("alice")));
  await Promise.resolve();
  expect(document.querySelector('input[name="username"]')).toHaveValue("bob");
  history.replaceState(null, "", "/login?next=%2Fapp%2Fnotes");
  const length = history.length;
  window.dispatchEvent(new PopStateEvent("popstate"));
  await waitFor(() => expect(document.querySelector(".account")).not.toBeNull());
  expect(history.length).toBe(length);
});

it("shows a busy state, prevents duplicate login requests and restores the form after an authentication failure", async () => {
  document.body.innerHTML = form("alice").replace('<form ', '<form data-login-form ');
  const input = document.querySelector<HTMLInputElement>('input[type="password"]')!;
  input.value = "invalid-test-password";
  let finish!: (value: ReturnType<typeof response> & { status: number }) => void;
  const fetch = vi.fn().mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  vi.stubGlobal("fetch", fetch);
  const element = document.querySelector("form")!;
  const submit = () => element.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  expect(submit()).toBe(false);
  expect(element).toHaveAttribute("aria-busy", "true");
  expect(element.querySelector("button")).toBeDisabled();
  submit();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(fetch.mock.calls[0]![1]).toMatchObject({ method: "POST", cache: "no-store", redirect: "error", headers: { Accept: "application/json" } });
  finish({ ...response(form("alice")), ok: false, status: 401 });
  await waitFor(() => expect(document.querySelector("form")).not.toBe(element));
  expect(input.value).toBe("");
  expect(document.querySelector("button")).not.toBeDisabled();
  expect(document.querySelector('input[type="password"]')).toHaveFocus();
});

it("uses View Transitions for view replacement when supported", async () => {
  const transition = vi.fn((update: () => void) => {
    const done = Promise.resolve().then(update);
    return { ready: Promise.resolve(), updateCallbackDone: done, finished: done, skipTransition: vi.fn() };
  });
  Object.defineProperty(document, "startViewTransition", { configurable: true, value: transition });
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response(form("alice"))));
  click(".account");
  await waitFor(() => expect(document.querySelector('input[name="username"]')).toHaveValue("alice"));
  expect(transition).toHaveBeenCalledTimes(1);
  Reflect.deleteProperty(document, "startViewTransition");
});
