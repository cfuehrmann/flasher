import { useState } from "react";
import { ToastContainer } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

import { getNarratives } from "./AppNarratives";
import { CardView } from "./CardView";
import { GroomView } from "./GroomView";
import { AppNarratives, AppState, RouterState } from "./types";
import { LoginView } from "./LoginView";
import { RecoverView } from "./RecoverView";
import { QuizView } from "./QuizView";

function App() {
  const [state, setState] = useState<AppState>({
    routerState: { route: "Prompt" },
    isContactingServer: true,
  });

  const narratives = getNarratives(setState);
  const { isContactingServer, ...pageProps } = {
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

export default App;

function Router(props: { routerState: RouterState } & AppNarratives) {
  const routerState = props.routerState;

  switch (routerState.route) {
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
          handleApi={props.handleApi}
          onGoToPrompt={props.goToPrompt}
          {...routerState.card}
        />
      );
    default: {
      return <CardView>Should never happen!</CardView>;
    }
  }
}
