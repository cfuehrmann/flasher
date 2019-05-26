import "github-markdown-css";
import "katex/dist/katex.min.css";
import "w3-css";

import * as React from "preact";

import { App } from "./App";

window.onload = () => {
  React.render(<App />, document.getElementById("root")!);
};
