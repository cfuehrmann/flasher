import { api } from "./Api";

import { AbandonButton, ButtonBar, SaveButton } from "./Buttons";
import { ApiHandler, Card } from "./types";
import { useAutoSave } from "./useAutoSave";

type Props = Card & ApiHandler & { onGoToPrompt: () => void };

export function RecoverView(props: Props) {
  const { id, prompt, solution } = props;

  const {
    card,
    setPrompt,
    setSolution,
    clearAutoSaveInterval,
    startAutoSaveInterval,
  } = useAutoSave({ id, prompt, solution }, props.handleApi(api.writeAutoSave));

  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <div className="w3-panel w3-yellow">
        <h3>Warning!</h3>
        <p>
          Please decide what to do with this unfinished edit from the previous
          session.
        </p>
      </div>
      <br />
      <div className="w3-card">
        <input
          className="w3-input"
          type="text"
          onChange={(event) => setPrompt(event.target.value)}
          value={card.prompt}
        />
        <br />
        <textarea
          className="w3-input"
          rows={17}
          onChange={(event) => setSolution(event.target.value)}
          value={card.solution}
        />
        <br />
        <br />
        <ButtonBar>
          <AbandonButton
            onClick={() => {
              clearAutoSaveInterval();
              props.handleApi(async () => {
                try {
                  await api.deleteAutoSave();
                  props.onGoToPrompt();
                } catch (_) {
                  startAutoSaveInterval();
                }
              })();
            }}
          />
          <SaveButton
            onClick={() => {
              clearAutoSaveInterval();
              props.handleApi(async () => {
                try {
                  await api.updateCard(card);
                  props.onGoToPrompt();
                } catch (_) {
                  startAutoSaveInterval();
                }
              })();
            }}
          />
        </ButtonBar>
        <br />
      </div>
      <br />
    </div>
  );
}
