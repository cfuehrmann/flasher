import * as React from "react";
import { useState, useEffect } from "react";
import { ToastContainer } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

import { getNarratives, initialize } from "./AppNarratives";
import { CardView } from "./CardView";
import { GroomView } from "./GroomView";
import { AppNarratives, AppState, RouterState } from "./types";
import { LoginView } from "./LoginView";
import { RecoverView } from "./RecoverView";
import { QuizView } from "./QuizView";

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
      <ToastContainer />
    </>
  );
}

function Router(props: { routerState: RouterState } & AppNarratives) {
  const routerState = props.routerState;

  switch (routerState.route) {
    case "Starting":
      return <p>Starting...</p>;
    case "Login":
      return <LoginView userName="" password="" onOk={props.login} />;
    case "Prompt":
      return (
        <QuizView handleApi={props.handleApi} onGoToGroom={props.goToGroom} />
      );
    case "Groom":
      return (
        <GroomView
          handleApi={props.handleApi}
          onGoToPrompt={props.goToPrompt}
        />
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
