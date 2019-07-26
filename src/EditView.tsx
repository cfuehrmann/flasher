import * as React from "react";

import {
  AsNewButton,
  ButtonBar,
  CancelButton,
  DeleteButton,
  SaveButton,
} from "./Buttons";
import { Card } from "./types";
import { useState } from "react";

type Props = Card & {
  onDelete: (id: string) => void;
  onSaveAsNew: (card: Card) => void;
  onCancel: () => void;
  onSave: (card: Card) => void;
};

export function EditView(props: Props) {
  const [card, setCard] = useState({
    id: props.id,
    prompt: props.prompt,
    solution: props.solution,
  });

  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <br />
      <div className="w3-card">
        <input
          className="w3-input"
          type="text"
          onInput={event => setPrompt((event.target as HTMLInputElement).value)}
          value={card.prompt}
        />
        <br />
        <textarea
          className="w3-input"
          rows={17}
          onInput={event =>
            setSolution((event.target as HTMLTextAreaElement).value)
          }
          value={card.solution}
        />
        <br />
        <br />
        <ButtonBar>
          <DeleteButton width="26%" onClick={() => props.onDelete(card.id)} />
          <AsNewButton width="26%" onClick={() => props.onSaveAsNew(card)} />
          <CancelButton width="26%" onClick={props.onCancel} />
          <SaveButton width="22%" onClick={() => props.onSave(card)} />
        </ButtonBar>
        <br />
      </div>
      <br />
    </div>
  );

  // Narratives for this component. Not pulled out because too trivial
  function setPrompt(prompt: string) {
    setCard({ ...card, prompt });
  }
  function setSolution(solution: string) {
    setCard({ ...card, solution });
  }
}
