import { afterEach, describe, expect, test, vi } from "vitest";

class FakeWorker extends EventTarget {
  state: ServiceWorkerState = "installing";
  postMessage = vi.fn();
}

class FakeRegistration extends EventTarget {
  active: FakeWorker | null = null;
  waiting: FakeWorker | null = null;
  installing: FakeWorker | null = null;
  update = vi.fn(async () => undefined);
}

afterEach(() => { vi.resetModules(); });

describe("frontend update state", () => {
  test("stays silent during the first installation", async () => {
    const client = await import("./frontend-updates");
    const registration = new FakeRegistration();
    const worker = new FakeWorker();
    registration.installing = worker;
    client.watchFrontendUpdates(registration as unknown as ServiceWorkerRegistration);
    expect(client.frontendUpdateState().phase).toBe("idle");
    registration.dispatchEvent(new Event("updatefound"));
    expect(client.frontendUpdateState().phase).toBe("idle");
    worker.state = "installed";
    worker.dispatchEvent(new Event("statechange"));
    expect(client.frontendUpdateState().phase).toBe("idle");
  });

  test("announces an update, then waits for the reload decision", async () => {
    const client = await import("./frontend-updates");
    const registration = new FakeRegistration();
    const active = new FakeWorker();
    active.state = "activated";
    registration.active = active;
    const worker = new FakeWorker();
    registration.installing = worker;
    client.watchFrontendUpdates(registration as unknown as ServiceWorkerRegistration);
    expect(client.frontendUpdateState().phase).toBe("downloading");
    worker.state = "installed";
    worker.dispatchEvent(new Event("statechange"));
    expect(client.frontendUpdateState().phase).toBe("ready");
    client.dismissFrontendUpdate();
    expect(client.frontendUpdateState().phase).toBe("idle");
  });

  test("detects a waiting worker from an earlier session", async () => {
    const client = await import("./frontend-updates");
    const registration = new FakeRegistration();
    const active = new FakeWorker();
    active.state = "activated";
    registration.active = active;
    registration.waiting = new FakeWorker();
    client.watchFrontendUpdates(registration as unknown as ServiceWorkerRegistration);
    expect(client.frontendUpdateState().phase).toBe("ready");
  });
});
