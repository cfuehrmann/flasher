import * as React from "preact";
import "w3-css";

import { App } from "./App";

window.onload = () => {
  React.render(<App />, document.getElementById("root")!);
};
