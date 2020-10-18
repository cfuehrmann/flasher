import * as React from "react";
import { ButtonBar, CancelButton, OkButton } from "./Buttons";

export function Modal(props: {
  text: string;
  okAction: () => void;
  cancelAction: () => void;
}) {
  return (
    <div className="w3-modal" style={{ display: "block" }}>
      <div className="w3-modal-content">
        <div className="w3-container">
          <p>{props.text}</p>
          <ButtonBar>
            <OkButton onClick={props.okAction}></OkButton>
            <CancelButton onClick={props.cancelAction}></CancelButton>
          </ButtonBar>
          <br />
        </div>
      </div>
    </div>
  );
}
