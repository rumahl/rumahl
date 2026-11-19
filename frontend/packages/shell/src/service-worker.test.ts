import { afterEach, expect, test, vi } from "vitest";

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); vi.resetModules(); });
test("failed registration can be retried and exposes the browser error", async () => {
  const registration = { scope: "https://os.test/" };
  const register = vi.fn().mockRejectedValueOnce(new Error("Certificate rejected")).mockResolvedValue(registration);
  vi.stubGlobal("isSecureContext", true);
  vi.stubGlobal("navigator", { serviceWorker: { register } });
  const client = await import("./service-worker");
  await expect(client.registerServiceWorker()).rejects.toThrow("Certificate rejected");
  expect(client.serviceWorkerError).toBe("Certificate rejected");
  await expect(client.registerServiceWorker()).resolves.toBe(registration);
  expect(client.serviceWorkerError).toBe("");
  expect(register).toHaveBeenCalledWith("/service-worker.js", { scope: "/", updateViaCache: "none" });
});
test("concurrent callers share one registration", async () => {
  const register = vi.fn().mockResolvedValue({});
  vi.stubGlobal("isSecureContext", true);
  vi.stubGlobal("navigator", { serviceWorker: { register } });
  const client = await import("./service-worker");
  await Promise.all([client.registerServiceWorker(), client.registerServiceWorker()]);
  expect(register).toHaveBeenCalledTimes(1);
});
test("insecure contexts fail without invoking registration", async () => {
  const register = vi.fn();
  vi.stubGlobal("isSecureContext", false);
  vi.stubGlobal("navigator", { serviceWorker: { register } });
  const client = await import("./service-worker");
  await expect(client.registerServiceWorker()).rejects.toThrow("HTTPS");
  expect(register).not.toHaveBeenCalled();
});
