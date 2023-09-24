import { ButtonBar, CancelButton, OkButton } from "./Buttons";

export function Modal(props: {
  text: string;
  okAction: () => Promise<void>;
  cancelAction: () => Promise<void>;
}) {
  return (
    <div className="w3-modal" style={{ display: "block" }}>
      <div className="w3-modal-content">
        <div className="w3-container">
          <p>{props.text}</p>
          <ButtonBar>
            <OkButton onClick={() => void props.okAction()}></OkButton>
            <CancelButton
              onClick={() => void props.cancelAction()}
            ></CancelButton>
          </ButtonBar>
          <br />
        </div>
      </div>
    </div>
  );
}
