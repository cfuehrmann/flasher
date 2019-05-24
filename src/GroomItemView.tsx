import MarkDownIt = require("markdown-it");
import * as React from "preact";

import { Card } from "./types";

type Props = Card & { onEdit: () => void; onBack: () => void };

export function GroomItemView(props: Props) {
  return (
    <>
      <CardView>
        <Prompt value={props.prompt} />
        <br />
        <Solution
          solution={props.solution}
          onEdit={props.onEdit}
          onBack={props.onBack}
        />
        <br />
      </CardView>
    </>
  );
}

function CardView(props: { children: React.ComponentChildren }) {
  return (
    <div className="w3-container">
      <br />
      <div className="w3-card">{props.children}</div>
      <br />
    </div>
  );
}

function Prompt(props: { value: string }) {
  return (
    <header className="w3-container">
      <h3>{props.value}</h3>
    </header>
  );
}

function Solution(props: {
  solution: string;
  onEdit: () => void;
  onBack: () => void;
}) {
  const { solution, onEdit, onBack } = props;

  return (
    <>
      <div
        className="w3-container markdown-body"
        dangerouslySetInnerHTML={{ __html: new MarkDownIt().render(solution) }}
      />
      <br />
      <br />
      <div className="w3-container">
        <div className="w3-bar">
          <button
            className="w3-bar-item w3-button w3-dark-grey"
            style={{ width: "50%" }}
            onClick={onEdit}
          >
            Edit
          </button>
          <button
            className="w3-bar-item w3-button w3-red"
            style={{ width: "50%" }}
            onClick={onBack}
          >
            Back
          </button>
        </div>
      </div>
    </>
  );
}
