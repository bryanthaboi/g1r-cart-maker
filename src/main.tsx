import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { StoreProvider } from "./state/store";
import "./styles/tokens.css";
import "./styles/app.css";

const host = document.getElementById("root");
if (!host) throw new Error("index.html has no #root element to mount into.");

createRoot(host).render(
  <StrictMode>
    <StoreProvider>
      <App />
    </StoreProvider>
  </StrictMode>,
);
