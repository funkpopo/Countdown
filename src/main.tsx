import React from "react";
import ReactDOM from "react-dom/client";
import { LanguageProvider } from "./i18n";
import App from "./App";
import QuickView from "./QuickView";

const rootElement = document.getElementById("root") as HTMLElement;

if (window.location.pathname === "/quick-view") {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <LanguageProvider>
        <QuickView />
      </LanguageProvider>
    </React.StrictMode>,
  );
} else {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <LanguageProvider>
        <App />
      </LanguageProvider>
    </React.StrictMode>,
  );
}
