import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MixerSurface } from "./MixerSurface";
import "../styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <MixerSurface />
  </StrictMode>,
);
