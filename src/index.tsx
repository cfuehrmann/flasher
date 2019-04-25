import * as React from "/web_modules/preact.js";
import * as Hooks from "/web_modules/preact/hooks.js";

function Greetings(props: { name: string }) {
  const [count, setCount] = Hooks.useState(0);

  Hooks.useEffect(() => {
    document.title = count + "";
  });
  return (
    <button onClick={() => setCount(count + 1)}>Greetings, {count}! </button>
  );
}
React.render(
  React.createElement(Greetings, { name: "Chris" }),
  document.getElementById("root")!
);
