import * as React from "react";
import { useState, useEffect, useRef } from "react";

import {
  AsNewButton,
  ButtonBar,
  CancelButton,
  DeleteButton,
  SaveButton,
} from "./Buttons";
import { Card } from "./types";

type Props = Card & {
  onDelete: (id: string) => void;
  onSaveAsNew: (card: Card) => void;
  onCancel: () => void;
  onSave: (card: Card) => void;
  saveSnapshot: (card: Card) => Promise<void>;
};

export function EditView(props: Props) {
  const [card, setCard] = useState({
    id: props.id,
    prompt: props.prompt,
    solution: props.solution,
  });

  const isSaving = useRef(false);
  const cardRef = useRef(card);

  useEffect(() => {
    cardRef.current = card;
  });

  useEffect(() => {
    console.log("start");
    const interval = setInterval(async () => {
      if (isSaving.current) {
        return;
      }
      isSaving.current = true;
      await props.saveSnapshot(cardRef.current);
      isSaving.current = false;
    }, 500);
    return () => {
      clearInterval(interval);
      console.log("stop");
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  // Narratives for this component. Not pulled out because too trivial
  function setPrompt(prompt: string) {
    setCard({ ...card, prompt });
  }
  function setSolution(solution: string) {
    setCard({ ...card, solution });
  }
}
