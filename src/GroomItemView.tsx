import * as React from "preact";

import {
  BackButton,
  ButtonBar,
  DisableButton,
  EditButton,
  EnableButton,
} from "./Buttons";
import { CardView } from "./CardView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";
import { GroomCard } from "./types";

type Props = GroomCard & {
  onEnable: (id: string) => void;
  onDisable: (id: string) => void;
  onEdit: () => void;
  onBack: () => void;
};

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
          {props.disabled ? (
            <EnableButton
              width="33%"
              onClick={() => props.onEnable(props.id)}
            />
          ) : (
            <DisableButton
              width="33%"
              onClick={() => props.onDisable(props.id)}
            />
          )}
          <EditButton width="33%" onClick={props.onEdit} />
          <BackButton width="34%" onClick={props.onBack} />
        </ButtonBar>
        <br />
      </CardView>
    </>
  );
}
