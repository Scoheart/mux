import "./index.css";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ToastProvider } from "./components/Toast";
import { applyTheme, getInitialTheme } from "./lib/theme";
import "./i18n";
import { LocaleProvider } from "./i18n/LocaleProvider";

// Apply the saved/system theme before first paint to avoid a flash.
applyTheme(getInitialTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LocaleProvider>
      <ToastProvider>
        <App />
      </ToastProvider>
    </LocaleProvider>
  </React.StrictMode>,
);
