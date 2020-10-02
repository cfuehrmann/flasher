import * as React from "react";

import { ButtonBar, CancelButton, SaveButton } from "./Buttons";
import { Card } from "./types";
import { useAutoSave } from "./useAutoSave";

type Props = Card & {
  onCancel: () => void;
  onSave: (card: Card) => void;
  writeAutoSave: (card: Card) => Promise<void>;
};

export function EditView(props: Props) {
  const { id, prompt, solution } = props;

  const { card, setPrompt, setSolution } = useAutoSave(
    { id, prompt, solution },
    props.writeAutoSave,
  );

  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <br />
      <div className="w3-card">
        <input
          className="w3-input"
          type="text"
          onChange={(event) => setPrompt(event.target.value)}
          value={card.prompt}
        />
        <br />
        <textarea
          className="w3-input"
          rows={17}
          onChange={(event) => setSolution(event.target.value)}
          value={card.solution}
        />
        <br />
        <br />
        <ButtonBar>
          <CancelButton width="50%" onClick={props.onCancel} />
          <SaveButton width="50%" onClick={() => props.onSave(card)} />
        </ButtonBar>
        <br />
      </div>
      <br />
    </div>
  );
}
