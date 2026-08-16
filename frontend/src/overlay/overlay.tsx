import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { OverlaySurface } from "./OverlaySurface";
import "../styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <OverlaySurface />
  </StrictMode>,
);
