import { useState } from "react";

import {
  BackButton,
  ButtonBar,
  DeleteButton,
  DeleteHistoryButton,
  DisableButton,
  EditButton,
  EnableButton,
} from "./Buttons";
import { CardView } from "./CardView";
import { Modal } from "./Modal";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";
import { GroomCard } from "./types";

type Props = GroomCard & {
  onDeleteHistory: () => void;
  onDelete: () => void;
  onEnable: (id: string) => void;
  onDisable: (id: string) => void;
  onEdit: () => void;
  onBack: () => void;
};

type ModalMode =
  | { modal: "none" }
  | { modal: "show"; text: string; action: () => void };

export function GroomItemView(props: Props) {
  const [modalMode, setModalMode] = useState<ModalMode>({ modal: "none" });

  return (
    <>
      <CardView>
        {modalMode.modal === "show" ? (
          <Modal
            text={modalMode.text}
            okAction={modalMode.action}
            cancelAction={() => setModalMode({ modal: "none" })}
          ></Modal>
        ) : (
          ""
        )}
        <PromptView value={props.prompt} />
        <br />
        <SolutionView solution={props.solution} />
        <br />
        <br />
        <ButtonBar>
          {props.state === "new" ? (
            <DeleteButton
              width="50%"
              onClick={() =>
                setModalMode({
                  modal: "show",
                  text: "Really delete this card?",
                  action: () => {
                    setModalMode({ modal: "none" });
                    props.onDelete();
                  },
                })
              }
            />
          ) : (
            <DeleteHistoryButton
              width="50%"
              onClick={() =>
                setModalMode({
                  modal: "show",
                  text: "Really delete this card's history?",
                  action: () => {
                    setModalMode({ modal: "none" });
                    props.onDeleteHistory();
                  },
                })
              }
            />
          )}
          {props.disabled ? (
            <EnableButton
              width="50%"
              onClick={() => props.onEnable(props.id)}
            />
          ) : (
            <DisableButton
              width="50%"
              onClick={() => props.onDisable(props.id)}
            />
          )}
        </ButtonBar>
        <ButtonBar>
          <EditButton width="50%" onClick={props.onEdit} />
          <BackButton width="50%" onClick={props.onBack} />
        </ButtonBar>
        <br />
      </CardView>
    </>
  );
}
