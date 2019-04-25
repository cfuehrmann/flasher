import * as React from "/web_modules/preact.js";
import * as Hooks from "/web_modules/preact/hooks.js";

import { getNarratives, initialize } from "./AppNarratives.js";
import { CreateView } from "./CreateView.js";
import { EditView } from "./EditView.js";
import { GroomView } from "./GroomView.js";
import { AppNarratives, AppState, RouterState } from "./types";

export function App() {
  const [state, setState] = Hooks.useState<AppState>({
    routerState: { route: "Starting" },
    isFetching: false,
    apiError: null
  });

  Hooks.useEffect(() => {
    initialize(setState);
  }, []);

  const narratives = getNarratives(setState);
  const { apiError, isFetching, ...pageProps } = {
    ...state,
    ...narratives
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
  } & AppNarratives
) {
  const routerState = props.routerState;

  switch (routerState.route) {
    case "Starting":
      return <p>Starting...</p>;
    case "Prompt": {
      return (
        <>
          <Menu
            onGoToCreate={() => props.goToCreate(routerState)}
            onGoToGroom={props.goToGroom}
          />
          <CardView>
            <Prompt value={routerState.card.prompt} />
            <br />
            <ShowButton onClick={() => props.showSolution(routerState.card)} />
            <br />
          </CardView>
        </>
      );
    }
    case "Solution": {
      const { id, prompt, solution } = routerState.card;

      return (
        <>
          <Menu
            onGoToCreate={() => props.goToCreate(routerState)}
            onGoToGroom={props.goToGroom}
          />
          <CardView>
            <Prompt value={prompt} />
            <br />
            <Solution
              solution={solution}
              onEdit={() => props.editSolution(routerState)}
              onOk={() => props.setOk(id)}
              onFailed={() => props.setFailed(id)}
            />
            <br />
          </CardView>
        </>
      );
    }
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
    case "Create":
      return (
        <CreateView
          onCreate={routerState.onSave}
          onCancel={routerState.onCancel}
        />
      );
    case "Groom":
      return (
        <GroomView
          onGoToPrompt={props.goToPrompt}
          onGoToCreate={() => props.createFromGroom(routerState)}
          onChangeInput={props.setCards}
          onEdit={props.editGroomItem(routerState)}
          searchText={routerState.searchText}
          cards={routerState.cards}
        />
      );
    case "Done":
      return (
        <>
          <Menu
            onGoToCreate={() => props.goToCreate(routerState)}
            onGoToGroom={props.goToGroom}
          />
          <CardView>Congrats, there are no due cards!</CardView>
        </>
      );
    default: {
      return <CardView>Should never happen!</CardView>;
    }
  }
}

function Menu(props: { onGoToCreate: () => void; onGoToGroom: () => void }) {
  return (
    <div className="w3-bar">
      <button className="w3-bar-item w3-button" onClick={props.onGoToCreate}>
        Create
      </button>
      <button className="w3-bar-item w3-button" onClick={props.onGoToGroom}>
        Groom
      </button>
    </div>
  );
}

function CardView(props: { children: React.ComponentChildren }) {
  return (
    <div className="w3-container" style={{ whiteSpace: "pre-wrap" }}>
      <br />
      <div className="w3-card">{props.children}</div>
      <br />
    </div>
  );
}

function Prompt(props: { value: string }) {
  return (
    <header className="w3-container">
      <h3>{props.value}</h3>
    </header>
  );
}

function Solution(props: {
  solution: string;
  onEdit: () => void;
  onOk: () => void;
  onFailed: () => void;
}) {
  const {
    solution,
    onEdit: goToEdit,
    onOk: setOk,
    onFailed: setFailed
  } = props;

  return (
    <>
      <div className="w3-container">{solution}</div>
      <br />
      <br />
      <div className="w3-container">
        <div className="w3-bar">
          <button
            className="w3-bar-item w3-button w3-dark-grey"
            style={{ width: "33%" }}
            onClick={goToEdit}
          >
            Edit
          </button>
          <button
            className="w3-bar-item w3-button w3-green"
            style={{ width: "34%" }}
            onClick={setOk}
          >
            Ok
          </button>
          <button
            className="w3-bar-item w3-button w3-red"
            style={{ width: "33%" }}
            onClick={setFailed}
          >
            Failed
          </button>
        </div>
      </div>
    </>
  );
}

function ShowButton(props: { onClick: () => void }) {
  return (
    <div className="w3-container">
      <button
        className="w3-button w3-block w3-dark-grey"
        onClick={props.onClick}
      >
        Show
      </button>
    </div>
  );
}
