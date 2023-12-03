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
  onDeleteHistory: () => Promise<void>;
  onDelete: () => Promise<void>;
  onEnable: (id: string) => Promise<void>;
  onDisable: (id: string) => Promise<void>;
  onEdit: () => void;
  onBack: () => void;
};

type ModalMode =
  | { modal: "none" }
  | { modal: "show"; text: string; action: () => Promise<void> };

export function GroomItemView(props: Props) {
  const [modalMode, setModalMode] = useState<ModalMode>({ modal: "none" });

  return (
    <>
      <CardView>
        {modalMode.modal === "show" ? (
          <Modal
            text={modalMode.text}
            okAction={modalMode.action}
            cancelAction={async () => {
              setModalMode({ modal: "none" });
              return Promise.resolve();
            }}
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
          {props.state === "New" ? (
            <DeleteButton
              width="50%"
              onClick={() =>
                setModalMode({
                  modal: "show",
                  text: "Really delete this card?",
                  action: async () => {
                    setModalMode({ modal: "none" });
                    await props.onDelete();
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
                  action: async () => {
                    setModalMode({ modal: "none" });
                    await props.onDeleteHistory();
                  },
                })
              }
            />
          )}
          {props.disabled ? (
            <EnableButton
              width="50%"
              onClick={() => void props.onEnable(props.id)}
            />
          ) : (
            <DisableButton
              width="50%"
              onClick={() => void props.onDisable(props.id)}
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
