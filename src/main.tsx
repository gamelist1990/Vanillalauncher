import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

function hideBootSplash() {
  const splash = document.getElementById("boot-splash");
  if (!splash) {
    return;
  }

  splash.classList.add("is-hiding");
  window.setTimeout(() => splash.remove(), 220);
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

window.requestAnimationFrame(() => {
  window.requestAnimationFrame(hideBootSplash);
});
