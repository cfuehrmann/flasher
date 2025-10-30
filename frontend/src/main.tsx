import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.tsx";
import "w3-css";
import "./custom.css";
import "katex/dist/katex.min.css";
import "github-markdown-css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
