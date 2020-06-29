import * as React from "react";

import { AbandonButton, ButtonBar, SaveButton } from "./Buttons";
import { Card } from "./types";
import { useAutoSave } from "./useAutoSave";

type Props = Card & {
  onAbandon: () => void;
  onSave: (card: Card) => void;
  writeAutoSave: (card: Card) => Promise<void>;
};

export function RecoverView(props: Props) {
  const { id, prompt, solution } = props;

  const { card, setPrompt, setSolution } = useAutoSave(
    { id, prompt, solution },
    props.writeAutoSave,
  );

  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <div className="w3-panel w3-yellow">
        <h3>Warning!</h3>
        <p>
          Please decide what to do with this unfinished edit from the previous
          session.
        </p>
      </div>
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
          <AbandonButton onClick={props.onAbandon} />
          <SaveButton onClick={() => props.onSave(card)} />
        </ButtonBar>
        <br />
      </div>
      <br />
    </div>
  );
}
