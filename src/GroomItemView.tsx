import * as React from "preact";

import { CardView } from "./CardView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";

import { Card } from "./types";

type Props = Card & { onEdit: () => void; onBack: () => void };

export function GroomItemView(props: Props) {
  return (
    <>
      <CardView>
        <PromptView value={props.prompt} />
        <br />
        <SolutionView solution={props.solution} />
        <br />
        <br />
        <Buttons onEdit={props.onEdit} onBack={props.onBack} />
        <br />
      </CardView>
    </>
  );
}

function Buttons(props: { onEdit: () => void; onBack: () => void }) {
  return (
    <div className="w3-container">
      <div className="w3-bar">
        <button
          className="w3-bar-item w3-button w3-dark-grey"
          style={{ width: "50%" }}
          onClick={props.onEdit}
        >
          Edit
        </button>
        <button
          className="w3-bar-item w3-button w3-red"
          style={{ width: "50%" }}
          onClick={props.onBack}
        >
          Back
        </button>
      </div>
    </div>
  );
}
