import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import QuickView from "./QuickView";

const rootElement = document.getElementById("root") as HTMLElement;

if (window.location.pathname === "/quick-view") {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <QuickView />
    </React.StrictMode>,
  );
} else {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
