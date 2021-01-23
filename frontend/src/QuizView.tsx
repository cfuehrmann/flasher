import React, { useEffect, useState } from "react";

import { api } from "./Api";
import {
  ButtonBar,
  EditButton,
  FailedButton,
  OkButton,
  RefreshButton,
  ShowButton,
  TextButton,
} from "./Buttons";
import { CardView } from "./CardView";
import { EditView } from "./EditView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";
import { ApiHandler, Card } from "./types";

type State =
  | { mode: "fetching" }
  | { mode: "prompt"; card: Card }
  | { mode: "solution"; card: Card }
  | { mode: "edit"; card: Card }
  | { mode: "done" };

export function QuizView(props: ApiHandler & { onGoToGroom: () => void }) {
  const [state, setState] = useState<State>({ mode: "fetching" });

  useEffect(
    findNextCard,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  switch (state.mode) {
    case "fetching":
      return (
        <>
          <Menu />
          <CardView>
            <br />
            <div className="w3-container">Trying to fetch the next card.</div>
            <br />
            <ButtonBar>
              <RefreshButton width="100%" onClick={findNextCard} />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    case "prompt":
      return (
        <>
          <Menu />
          <CardView>
            <PromptView value={state.card.prompt} />
            <br />
            <ButtonBar>
              <ShowButton
                width="100%"
                onClick={() => setState({ ...state, mode: "solution" })}
              />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    case "solution":
      return (
        <>
          <Menu />
          <CardView>
            <PromptView value={state.card.prompt} />
            <br />
            <SolutionView solution={state.card.solution} />
            <br />
            <br />
            <ButtonBar>
              <EditButton
                width="33%"
                onClick={() => setState({ ...state, mode: "edit" })}
              />
              <OkButton
                width="34%"
                onClick={props.handleApi(async () => {
                  const card = await api.setOk(state.card.id);
                  setState(card ? { mode: "prompt", card } : { mode: "done" });
                })}
              />
              <FailedButton
                width="33%"
                onClick={props.handleApi(async () => {
                  const card = await api.setFailed(state.card.id);
                  setState(card ? { mode: "prompt", card } : { mode: "done" });
                })}
              />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    case "edit":
      return (
        <EditView
          {...state.card}
          onSave={(card, clearAutoSaveInterval, startAutoSaveInterval) => {
            clearAutoSaveInterval();
            props.handleApi(async () => {
              try {
                await api.updateCard(card);
                setState({ mode: "solution", card });
              } catch (_) {
                startAutoSaveInterval();
              }
            })();
          }}
          onCancel={(clearAutoSaveInterval, startAutoSaveInterval) => {
            clearAutoSaveInterval();
            props.handleApi(api.deleteAutoSave)();
            setState({ ...state, mode: "solution" });
          }}
          writeAutoSave={props.handleApi(api.writeAutoSave)}
        />
      );
    case "done":
      return (
        <>
          <Menu />
          <CardView>
            <br />
            <div className="w3-container">
              Congrats, there are no due cards!
            </div>
            <br />
            <ButtonBar>
              <RefreshButton width="100%" onClick={findNextCard} />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
  }

  function findNextCard() {
    props.handleApi(async () => {
      const card = await api.findNextCard();
      setState(card ? { mode: "prompt", card } : { mode: "done" });
      // setState({ mode: "done" });
    })();
  }

  function Menu() {
    return (
      <>
        <div className="w3-top">
          <div className="w3-bar w3-light-grey w3-border">
            <TextButton text="Groom" onClick={props.onGoToGroom} />
          </div>
        </div>
        <br />
        <br />
      </>
    );
  }
}
