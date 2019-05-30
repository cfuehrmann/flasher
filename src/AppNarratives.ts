import * as Hooks from "preact/hooks";

import { api } from "./Api";
import {
  AppNarratives,
  AppState,
  Card,
  GroomItemState,
  GroomState,
  RouterState,
  SolutionState,
} from "./types";

export function initialize(setState: Hooks.StateUpdater<AppState>) {
  return withApi(setState, () => promptNext(setState));
}

export const getNarratives = (
  setState: Hooks.StateUpdater<AppState>,
): AppNarratives => {
  return {
    showSolution,
    setOk,
    setFailed,
    editSolution,
    createFromPrompt,
    createFromGroom,
    goToGroom,
    goToPrompt,
    setCards,
    groomItem,
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
      onSave: saveAndShowSolution,
    });
  }

  function createFromPrompt(prevRouterState: RouterState) {
    createFromPromptLoop(prevRouterState, "", "");
  }

  function createFromPromptLoop(
    prevRouterState: RouterState,
    prompt: string,
    solution: string,
  ) {
    setRouterState({
      route: "Create",
      prompt,
      solution,
      onCancel: () => setRouterState(prevRouterState),
      onSave: (p: string, s: string) =>
        checkCreatedFromPrompt(p, s, prevRouterState),
    });
  }

  function checkCreatedFromPrompt(
    prompt: string,
    solution: string,
    prevRouterState: RouterState,
  ) {
    setRouterState({
      route: "CheckCreated",
      prompt,
      solution,
      onCancel: () => setRouterState(prevRouterState),
      onEdit: () => createFromPromptLoop(prevRouterState, prompt, solution),
      onSave: () =>
        withApi(setState, async () => {
          await api.createCard(prompt, solution);
          setRouterState(prevRouterState);
        }),
    });
  }

  function createFromGroom(groomState: GroomState) {
    createFromGroomLoop(groomState, "", "");
  }

  function createFromGroomLoop(
    groomState: GroomState,
    prompt: string,
    solution: string,
  ) {
    setRouterState({
      route: "Create",
      prompt,
      solution,
      onCancel: () => setRouterState(groomState),
      onSave: (p: string, s: string) => checkCreatedFromGroom(p, s, groomState),
    });
  }

  function checkCreatedFromGroom(
    prompt: string,
    solution: string,
    groomState: GroomState,
  ) {
    setRouterState({
      route: "CheckCreated",
      prompt,
      solution,
      onCancel: () => setRouterState(groomState),
      onEdit: () => createFromGroomLoop(groomState, prompt, solution),
      onSave: () =>
        withApi(setState, async () => {
          await api.createCard(prompt, solution);
          const cards = await api.findCards(groomState.searchText);
          setRouterState({ ...groomState, cards });
        }),
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

  function groomItem(prevRouterState: GroomState) {
    return (id: string) =>
      withApi(setState, async () => {
        const card = await api.readCard(id);
        if (card === undefined) {
          return;
        }
        setRouterState(getGroomItemState(card, prevRouterState));
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

  function getGroomItemState(card: Card, prevRouterState: GroomState) {
    const groomItemState: GroomItemState = {
      route: "GroomItem",
      card,
      onEdit: () =>
        setRouterState({
          route: "Edit",
          card,
          onDelete: deleteAndGroom(prevRouterState.searchText),
          onSaveAsNew: saveGroomItem(false),
          onCancel: () => setRouterState(groomItemState),
          onSave: saveGroomItem(true),
        }),
      onBack: () => setRouterState(prevRouterState),
    };
    return groomItemState;

    function saveGroomItem(isMinor: boolean) {
      return (cardToSave: Card) => {
        withApi(setState, async () => {
          await api.updateCard(cardToSave, isMinor);
          setRouterState(getGroomItemState(cardToSave, prevRouterState));
        });
      };
    }
  }

  function setRouterState(routerState: RouterState) {
    setState(prevState => ({ ...prevState, routerState }));
  }
};

async function withApi(
  setState: Hooks.StateUpdater<AppState>,
  apiMethod: () => Promise<void>,
) {
  setState(prevState => ({ ...prevState, isFetching: true }));
  try {
    await apiMethod();
    setState(prevState => ({ ...prevState, isFetching: false }));
  } catch (e) {
    setState(prevState => ({
      ...prevState,
      apiError: e.message,
      isFetching: false,
    }));
  }
}

async function promptNext(setState: Hooks.StateUpdater<AppState>) {
  const card = await api.findNextCard();
  setState(prevState => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" },
  }));
}
