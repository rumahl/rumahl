import { SHELL_SNAPSHOT_ENDPOINT, type ShellRequest } from "../snapshot-client";
import { demoSnapshot } from "./snapshot";

export class DemoBackend {
  readonly request: ShellRequest = async (input, init) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";

    if (url === SHELL_SNAPSHOT_ENDPOINT && method === "GET") {
      return Response.json(demoSnapshot, {
        headers: { "Cache-Control": "no-store" },
        status: 200
      });
    }

    return Response.json({ error: "demo route not found" }, { status: 404 });
  };
}
