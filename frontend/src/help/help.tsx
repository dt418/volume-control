import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HelpSurface } from "./HelpSurface";
import "../styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <HelpSurface />
  </StrictMode>,
);
