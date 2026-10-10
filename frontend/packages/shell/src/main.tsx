import { registerServiceWorker } from "./service-worker";
import { startShell } from "./runtime";

void registerServiceWorker().catch(() => { /* Device settings display the failure. */ });
await startShell();
