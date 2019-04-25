import * as React from "/web_modules/preact.js";
import * as Hooks from "/web_modules/preact/hooks.js";

import { Card } from "./types";

type Props = Card & {
  onDelete: (id: string) => void;
  onSaveAsNew: (card: Card) => void;
  onCancel: () => void;
  onSave: (card: Card) => void;
};

export function EditView(props: Props) {
  const [card, setCard] = Hooks.useState({
    id: props.id,
    prompt: props.prompt,
    solution: props.solution
  });

  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <br />
      <div className="w3-card">
        <input
          className="w3-input"
          type="text"
          onChange={event =>
            setPrompt((event.target as HTMLInputElement).value)
          }
          value={card.prompt}
        />
        <br />
        <textarea
          className="w3-input"
          rows={17}
          onChange={event =>
            setSolution((event.target as HTMLTextAreaElement).value)
          }
          value={card.solution}
        />
        <br />
        <br />
        <div className="w3-container">
          <div className="w3-bar">
            <button
              className="w3-bar-item w3-button w3-red"
              style={{ width: "26%" }}
              onClick={event => props.onDelete(card.id)}
            >
              Delete
            </button>
            <button
              className="w3-bar-item w3-button w3-green"
              style={{ width: "26%" }}
              onClick={event => props.onSaveAsNew(card)}
            >
              As&nbsp;new
            </button>
            <button
              className="w3-bar-item w3-button w3-red"
              style={{ width: "26%" }}
              onClick={props.onCancel}
            >
              Cancel
            </button>
            <button
              className="w3-bar-item w3-button w3-green"
              style={{ width: "22%" }}
              onClick={event => props.onSave(card)}
            >
              Save
            </button>
          </div>
        </div>
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
