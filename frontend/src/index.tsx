import "w3-css";
import "katex/dist/katex.min.css";
import "github-markdown-css";

import { createRoot } from "react-dom/client";
import React from "react";
import { App } from "./App";
// import reportWebVitals from './reportWebVitals';

const container = document.getElementById("root");
const root = createRoot(container!);

root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
// reportWebVitals();
