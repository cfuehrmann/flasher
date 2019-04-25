type SetStateAction<S> = S | ((prevState: S) => S);
type Dispatch<A> = (value: A) => void;

import { api } from "./Api.js";
import {
  AppNarratives,
  AppState,
  Card,
  GroomState,
  RouterState,
  SolutionState
} from "./types";

export function initialize(setState: Dispatch<SetStateAction<AppState>>) {
  return withApi(setState, () => promptNext(setState));
}

export const getNarratives = (
  setState: Dispatch<SetStateAction<AppState>>
): AppNarratives => {
  return {
    showSolution,
    setOk,
    setFailed,
    editSolution,
    goToCreate,
    createFromGroom,
    goToGroom,
    goToPrompt,
    setCards,
    editGroomItem
  };

  function showSolution(card: Card) {
    setRouterState({ route: "Solution", card });
  }

  function setOk(id: string) {
    withApi(setState, async () => {
      await api.setOk(id);
      promptNext(setState);
    });
  }

  function setFailed(id: string) {
    withApi(setState, async () => {
      await api.setFailed(id);
      promptNext(setState);
    });
  }

  function editSolution(prevRouterState: SolutionState) {
    setRouterState({
      route: "Edit",
      card: prevRouterState.card,
      onDelete: deleteAndNext,
      onSaveAsNew: saveAsNewAndNext,
      onCancel: () => setRouterState(prevRouterState),
      onSave: saveAndShowSolution
    });
  }

  function goToCreate(prevRouterState: RouterState) {
    setRouterState({
      route: "Create",
      onCancel: () => setRouterState(prevRouterState),
      onSave: (prompt: string, solution: string) => {
        withApi(setState, async () => {
          await api.createCard(prompt, solution);
          setRouterState(prevRouterState);
        });
      }
    });
  }

  function createFromGroom(prevRouterState: GroomState) {
    setRouterState({
      route: "Create",
      onCancel: () => setRouterState(prevRouterState),
      onSave: (prompt: string, solution: string) => {
        withApi(setState, async () => {
          await api.createCard(prompt, solution);
          const cards = await api.findCards(prevRouterState.searchText);
          setRouterState({ ...prevRouterState, cards });
        });
      }
    });
  }

  function goToGroom() {
    setRouterState({ route: "Groom", searchText: "", cards: [] });
  }

  function goToPrompt() {
    promptNext(setState);
  }

  function setCards(searchText: string) {
    withApi(setState, async () => {
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  function editGroomItem(prevRouterState: GroomState) {
    return (id: string) =>
      withApi(setState, async () => {
        const card = await api.readCard(id);
        if (card === undefined) {
          return;
        }
        setRouterState({
          route: "Edit",
          card,
          onDelete: deleteAndGroom(prevRouterState.searchText),
          onSaveAsNew: saveAndGroom(prevRouterState.searchText, false),
          onCancel: () => setRouterState(prevRouterState),
          onSave: saveAndGroom(prevRouterState.searchText, true)
        });
      });
  }

  function deleteAndNext(id: string) {
    withApi(setState, async () => {
      await api.deleteCard(id);
      promptNext(setState);
    });
  }

  function saveAsNewAndNext(card: Card) {
    withApi(setState, async () => {
      await api.updateCard(card, false);
      promptNext(setState);
    });
  }

  function saveAndShowSolution(card: Card) {
    withApi(setState, async () => {
      await api.updateCard(card, true);
      setRouterState({ route: "Solution", card });
    });
  }

  function deleteAndGroom(searchText: string) {
    return (id: string) => {
      withApi(setState, async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
    };
  }

  function saveAndGroom(searchText: string, isMinor: boolean) {
    return (card: Card) => {
      withApi(setState, async () => {
        await api.updateCard(card, isMinor);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
    };
  }

  function setRouterState(routerState: RouterState) {
    setState(prevState => ({ ...prevState, routerState }));
  }
};

async function withApi(
  setState: Dispatch<SetStateAction<AppState>>,
  apiMethod: () => Promise<void>
) {
  setState(prevState => ({ ...prevState, isFetching: true }));
  try {
    await apiMethod();
    setState(prevState => ({ ...prevState, isFetching: false }));
  } catch (e) {
    setState(prevState => ({
      ...prevState,
      apiError: e.message,
      isFetching: false
    }));
  }
}

async function promptNext(setState: Dispatch<SetStateAction<AppState>>) {
  const card = await api.findNextCard();
  setState(prevState => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" }
  }));
}
