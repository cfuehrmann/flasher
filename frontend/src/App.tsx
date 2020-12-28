import * as React from "react";
import { useState, useEffect } from "react";
import { ToastContainer, toast } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

import { getNarratives, initialize } from "./AppNarratives";
import {
  ButtonBar,
  EditButton,
  FailedButton,
  OkButton,
  ShowButton,
  RefreshButton,
  TextButton,
} from "./Buttons";
import { CardView } from "./CardView";
import { EditView } from "./EditView";
import { GroomView } from "./GroomView";
import { PromptView } from "./PromptView";
import { SolutionView } from "./SolutionView";
import { AppNarratives, AppState, RouterState } from "./types";
import { LoginView } from "./LoginView";
import { RecoverView } from "./RecoverView";

export function App() {
  const [state, setState] = useState<AppState>({
    routerState: { route: "Starting" },
    isContactingServer: false,
    serverError: undefined,
  });

  useEffect(() => {
    initialize(setState);
  }, []);

  const narratives = getNarratives(setState);
  const { serverError, isContactingServer, ...pageProps } = {
    ...state,
    ...narratives,
  };

  return (
    <>
      <Router {...pageProps} />
      {isContactingServer ? <p>Contacting server...</p> : ""}
      {!isContactingServer && serverError ? showServerError(serverError) : ""}
      <ToastContainer />
    </>
  );
}

function showServerError(serverError: Error) {
  toast(
    typeof serverError.message === "string"
      ? serverError.message
      : "Unknown server error!",
    { type: "error", position: "bottom-right" },
  );
}

function Router(props: { routerState: RouterState } & AppNarratives) {
  const routerState = props.routerState;

  switch (routerState.route) {
    case "Starting":
      return <p>Starting...</p>;
    case "Login":
      return <LoginView userName="" password="" onOk={props.login} />;
    case "Prompt": {
      return (
        <>
          <Menu onGoToGroom={props.goToGroom} />
          <CardView>
            <PromptView value={routerState.card.prompt} />
            <br />
            <ButtonBar>
              <ShowButton
                width="100%"
                onClick={props.showSolution(routerState.card)}
              />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    }
    case "Done":
      return (
        <>
          <Menu onGoToGroom={props.goToGroom} />
          <CardView>
            <br />
            <div className="w3-container">
              Congrats, there are no due cards!
            </div>
            <br />
            <ButtonBar>
              <RefreshButton width="100%" onClick={props.goToPrompt} />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    case "Solution": {
      const { id, prompt, solution } = routerState.card;

      return (
        <>
          <Menu onGoToGroom={props.goToGroom} />
          <CardView>
            <PromptView value={prompt} />
            <br />
            <SolutionView solution={solution} />
            <br />
            <br />
            <ButtonBar>
              <EditButton
                width="33%"
                onClick={props.editSolution(routerState.card)}
              />
              <OkButton width="34%" onClick={props.setOk(id)} />
              <FailedButton width="33%" onClick={props.setFailed(id)} />
            </ButtonBar>
            <br />
          </CardView>
        </>
      );
    }
    case "Edit":
      return (
        <EditView
          {...routerState.card}
          onSave={props.saveAndShowSolution}
          onCancel={props.cancelEdit(routerState.card)}
          writeAutoSave={props.writeAutoSave}
        />
      );
    case "Groom":
      return (
        <GroomView handleApi={props.handle} onGoToPrompt={props.goToPrompt} />
      );

    case "Recover":
      return (
        <RecoverView
          {...routerState.card}
          onSave={props.saveRecovered}
          onAbandon={props.abandonRecovered}
          writeAutoSave={props.writeAutoSave}
        />
      );
    default: {
      return <CardView>Should never happen!</CardView>;
    }
  }
}

function Menu(props: { onGoToGroom: () => void }) {
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
