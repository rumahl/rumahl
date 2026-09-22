import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { fixtureSnapshot } from "./fixture";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("rumahl shell root element is missing");
}

createRoot(root).render(
  <StrictMode>
    <App snapshot={fixtureSnapshot} />
  </StrictMode>
);
