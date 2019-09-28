import * as React from "react";

import { getNarratives, initialize } from "./AppNarratives";
import {
  ButtonBar,
  EditButton,
  FailedButton,
  OkButton,
  ShowButton,
} from "./Buttons";
import { CardView } from "./CardView";
import { EditView } from "./EditView";
import { GroomItemView } from "./GroomItemView";
import { GroomView } from "./GroomView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";
import { AppNarratives, AppState, RouterState } from "./types";
import { useState, useEffect } from "react";
import { LoginView } from "./LoginView";

export function App() {
  const [state, setState] = useState<AppState>({
    routerState: { route: "Starting" },
    isFetching: false,
    apiError: null,
  });

  useEffect(() => {
    initialize(setState);
  }, []);

  const narratives = getNarratives(setState);
  const { apiError, isFetching, ...pageProps } = {
    ...state,
    ...narratives,
  };

  return (
    <>
      <Router {...pageProps} />
      {isFetching ? <p>Fetching data...</p> : ""}
      {!isFetching && apiError ? <p>Api error: {apiError}</p> : ""}
    </>
  );
}

function Router(
  props: {
    routerState: RouterState;
  } & AppNarratives,
) {
  const routerState = props.routerState;

  switch (routerState.route) {
    case "Starting":
      return <p>Starting...</p>;
    case "Login":
      return <LoginView userName="" password="" onOk={props.login} />;
    case "Prompt": {
      return (
        <>
          <Menu onGoToGroom={props.goToGroom} onGoToLogin={props.goToLogin} />
          <CardView>
            <PromptView value={routerState.card.prompt} />
            <br />
            <ButtonBar>
              <ShowButton
                width="100%"
                onClick={() => props.showSolution(routerState.card)}
              />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    }
    case "Solution": {
      const { id, prompt, solution } = routerState.card;

      return (
        <>
          <Menu onGoToGroom={props.goToGroom} onGoToLogin={props.goToLogin} />
          <CardView>
            <PromptView value={prompt} />
            <br />
            <SolutionView solution={solution} />
            <br />
            <br />
            <ButtonBar>
              <EditButton
                width="33%"
                onClick={() => props.editSolution(routerState)}
              />
              <OkButton width="34%" onClick={() => props.setOk(id)} />
              <FailedButton width="33%" onClick={() => props.setFailed(id)} />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    }
    case "GroomItem":
      return (
        <GroomItemView
          {...routerState.card}
          onEnable={routerState.onEnable}
          onDisable={routerState.onDisable}
          onBack={routerState.onBack}
          onEdit={routerState.onEdit}
        />
      );
    case "Edit":
      return (
        <EditView
          {...routerState.card}
          onDelete={routerState.onDelete}
          onSaveAsNew={routerState.onSaveAsNew}
          onSave={routerState.onSave}
          onCancel={routerState.onCancel}
        />
      );
    case "Groom":
      return (
        <GroomView
          onGoToPrompt={props.goToPrompt}
          onGoToCreate={() => props.create(routerState)}
          onChangeInput={props.setCards}
          onGroomItem={props.groomItem(routerState)}
          searchText={routerState.searchText}
          cards={routerState.cards}
        />
      );
    case "Done":
      return (
        <>
          <Menu onGoToGroom={props.goToGroom} onGoToLogin={props.goToLogin} />
          <CardView>Congrats, there are no due cards!</CardView>
        </>
      );
    default: {
      return <CardView>Should never happen!</CardView>;
    }
  }
}

function Menu(props: { onGoToGroom: () => void; onGoToLogin: () => void }) {
  return (
    <div className="w3-bar">
      <button className="w3-bar-item w3-button" onClick={props.onGoToGroom}>
        Groom
      </button>
      <button className="w3-bar-item w3-button" onClick={props.onGoToLogin}>
        Login
      </button>
    </div>
  );
}
