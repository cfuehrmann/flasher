import { ReactNode } from "react";

export function CardView(props: { children: ReactNode }) {
  return (
    <div className="w3-container">
      <br />
      <div className="w3-card">{props.children}</div>
      <br />
    </div>
  );
}
