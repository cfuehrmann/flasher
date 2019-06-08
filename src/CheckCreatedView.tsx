import * as React from "preact";

import { ButtonBar, CancelButton, CreateButton, EditButton } from "./Buttons";
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
        <ButtonBar>
          <CancelButton width="30%" onClick={props.onCancel} />
          <CreateButton
            width="30%"
            onClick={() => onCreate(prompt, solution)}
          />
          <EditButton width="40%" onClick={props.onEdit} />
        </ButtonBar>
        <br />
      </CardView>
    </>
  );
}
