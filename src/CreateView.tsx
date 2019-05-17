import * as React from "preact";
import * as Hooks from "preact/hooks";

type Props = {
  onCancel: () => void;
  onCreate: (prompt: string, solution: string) => void;
};

export function CreateView(props: Props) {
  const [card, setCard] = Hooks.useState({
    prompt: "",
    solution: "",
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
              style={{ width: "30%" }}
              onClick={props.onCancel}
            >
              Cancel
            </button>
            <button
              className="w3-bar-item w3-button w3-green"
              style={{ width: "40%" }}
              onClick={event => props.onCreate(card.prompt, card.solution)}
            >
              Create
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
    return setCard({ prompt, solution: card.solution });
  }
  function setSolution(solution: string) {
    return setCard({ prompt: card.prompt, solution });
  }
}
