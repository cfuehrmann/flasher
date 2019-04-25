import * as React from "./web_modules/preact.js";
import * as Hooks from "./web_modules/preact/hooks.js";

// window.onload = function() {
function Greetings(props: { name: string }) {
  const [count, setCount] = Hooks.useState(0);

  Hooks.useEffect(() => {
    document.title = count + "";
  });
  return React.h(
    "button",
    { onClick: () => setCount(count + 1) },
    "Greetings, " + count + "!"
  );
}
React.render(
  React.h(Greetings, { name: "Chris" }),
  document.getElementById("root")!
);
// };
