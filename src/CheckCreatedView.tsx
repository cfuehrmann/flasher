import * as React from "preact";
import { CardView } from "./CardView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";

type Props = {
  prompt: string;
  solution: string;
  onCancel: () => void;
  onCreate: (prompt: string, solution: string) => void;
  onEdit: () => void;
};

export function CheckCreatedView(props: Props) {
  const { prompt, solution, onCancel, onCreate, onEdit } = props;

  return (
    <>
      <CardView>
        <PromptView value={prompt} />
        <br />
        <SolutionView solution={solution} />
        <br />
        <br />
        <Buttons
          onCancel={onCancel}
          onCreate={() => onCreate(prompt, solution)}
          onEdit={onEdit}
        />
        <br />
      </CardView>
    </>
  );
}

function Buttons(props: {
  onCancel: () => void;
  onCreate: () => void;
  onEdit: () => void;
}) {
  return (
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
          style={{ width: "30%" }}
          onClick={props.onCreate}
        >
          Create
        </button>
        <button
          className="w3-bar-item w3-button w3-dark-grey"
          style={{ width: "40%" }}
          onClick={props.onEdit}
        >
          Edit
        </button>
      </div>
    </div>
  );
}
