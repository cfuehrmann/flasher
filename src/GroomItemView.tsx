import * as React from "preact";

import { BackButton, ButtonBar, EditButton } from "./Buttons";
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
        <ButtonBar>
          <EditButton width="50%" onClick={props.onEdit} />
          <BackButton width="50%" onClick={props.onBack} />
        </ButtonBar>
        <br />
      </CardView>
    </>
  );
}
