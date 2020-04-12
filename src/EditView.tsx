import * as React from "react";

import {
  AsNewButton,
  ButtonBar,
  CancelButton,
  DeleteButton,
  SaveButton,
} from "./Buttons";
import { Card } from "./types";
import { useAutoSave } from "./useAutoSave";

type Props = Card & {
  onDelete: (id: string) => void;
  onSaveAsNew: (card: Card) => void;
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
          onChange={event => setPrompt(event.target.value)}
          value={card.prompt}
        />
        <br />
        <textarea
          className="w3-input"
          rows={17}
          onChange={event => setSolution(event.target.value)}
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
}
