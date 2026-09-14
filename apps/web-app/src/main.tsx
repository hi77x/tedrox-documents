import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { setPlatform } from "@desktop/platform";
import { AppStateProvider } from "@desktop/state";
import { Shell } from "@desktop/App";
import { createWebPlatform } from "./web-platform";
import "@desktop/design/tokens.css";
import "./web.css";

setPlatform(createWebPlatform());

if ("serviceWorker" in navigator && import.meta.env.PROD) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register("./sw.js").catch(() => undefined);
  });
}

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <AppStateProvider>
      <Shell />
    </AppStateProvider>
  </StrictMode>,
);
