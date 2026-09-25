import { startShell } from "../runtime";
import { DemoBackend } from "./backend";

const backend = new DemoBackend();
await startShell({
  request: backend.request,
  openEvents: backend.openEvents,
  allowDevelopmentLoopback: true
});
