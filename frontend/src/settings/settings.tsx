import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { SettingsSurface } from "./SettingsSurface";
import "../styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsSurface />
  </StrictMode>,
);
