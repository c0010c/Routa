import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// Keep the browser menu available during development; hide it in packaged builds.
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
