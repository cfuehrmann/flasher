import * as React from "preact";
import * as Hooks from "preact/hooks";

import { ButtonBar, CancelButton, CreateButton } from "./Buttons";

type Props = {
  prompt: string;
  solution: string;
  onCancel: () => void;
  onCreate: (prompt: string, solution: string) => void;
};

export function CreateView(props: Props) {
  const [card, setCard] = Hooks.useState({
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
          <CancelButton width="50%" onClick={props.onCancel} />
          <CreateButton
            width="50%"
            onClick={() => props.onCreate(card.prompt, card.solution)}
          />
        </ButtonBar>
        <br />
      </div>
      <br />
    </div>
  );

  // Narratives for this component. Not pulled out because too trivial
  function setPrompt(prompt: string) {
    return setCard({ prompt, solution: card.solution });
  }
  function setSolution(solution: string) {
    return setCard({ prompt: card.prompt, solution });
  }
}
