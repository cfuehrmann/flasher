import * as React from "preact";

export function CardView(props: { children: React.ComponentChildren }) {
  return (
    <div className="w3-container">
      <br />
      <div className="w3-card">{props.children}</div>
      <br />
    </div>
  );
}
