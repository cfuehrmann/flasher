import { api } from "./Api";
import {
  AppNarratives,
  Card,
  GroomCard,
  GroomState,
  RouterState,
  SolutionState,
  SetStateType,
  AppState,
} from "./types";

export function initialize(setState: SetStateType) {
  return withErrorHandling(setState, async () => await promptNext(setState));
}

export const getNarratives = (setState: SetStateType): AppNarratives => {
  return {
    login,
    showSolution,
    setOk,
    setFailed,
    editSolution,
    create,
    goToGroom,
    goToPrompt,
    setCards,
    groomItem,
    writeAutoSave,
    deleteAndGroom,
    deleteAndNext,
    cancelEdit,
    cancelGroomEdit,
    disable,
    enable,
    groomEdit,
    backFromGromItem,
    saveGroomItem,
    saveAsNewAndNext,
    saveAndShowSolution,
  };

  function login(userName: string, password: string) {
    withErrorHandling(setState, async () => {
      const { autoSave } = await api.login(userName, password);

      if (autoSave) {
        setRouterState({
          route: "Recover",
          card: autoSave,
          onAbandon: async () => {
            await api.deleteAutoSave();
            await promptNext(setState);
          },
          onSave: async (card) => {
            await api.updateCard(card, true);
            await promptNext(setState);
          },
        });
      } else await promptNext(setState);
    });
  }

  function showSolution(card: Card) {
    setRouterState({ route: "Solution", card });
  }

  function setOk(id: string) {
    withErrorHandling(setState, async () => {
      await api.setOk(id);
      await promptNext(setState);
    });
  }

  function setFailed(id: string) {
    withErrorHandling(setState, async () => {
      await api.setFailed(id);
      await promptNext(setState);
    });
  }

  function editSolution(prevRouterState: SolutionState) {
    setRouterState({
      route: "Edit",
      card: prevRouterState.card,
    });
  }

  async function cancelEdit(card: Card) {
    setRouterState({ route: "Solution", card });
    await withErrorHandling(setState, async () => {
      await api.deleteAutoSave();
    });
  }

  async function cancelGroomEdit(card: GroomCard, searchText: string) {
    setRouterState({ route: "GroomItem", card, searchText });
    await withErrorHandling(setState, async () => {
      await api.deleteAutoSave();
    });
  }

  function create(groomState: GroomState) {
    withErrorHandling(setState, async () => {
      await api.createCard("New card", "");
      const cards = await api.findCards(groomState.searchText);
      setRouterState({ ...groomState, cards });
    });
  }

  function goToGroom() {
    setRouterState({ route: "Groom", searchText: "", cards: [] });
  }

  function goToPrompt() {
    withErrorHandling(setState, async () => {
      await promptNext(setState);
    });
  }

  function setCards(searchText: string) {
    withErrorHandling(setState, async () => {
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  function groomItem(searchText: string) {
    return (id: string) =>
      withErrorHandling(setState, async () => {
        const card = await api.readCard(id);
        if (card === undefined) {
          return;
        }
        setRouterState({
          route: "GroomItem",
          card,
          searchText,
        });
      });
  }

  function writeAutoSave(card: Card) {
    return withErrorHandling(setState, async () => {
      await api.writeAutoSave(card);
    });
  }

  function deleteAndNext(id: string) {
    return withErrorHandling(setState, async () => {
      await api.deleteCard(id);
      await promptNext(setState);
    });
  }

  function saveAsNewAndNext(card: Card) {
    return withErrorHandling(setState, async () => {
      await api.updateCard(card, false);
      await promptNext(setState);
    });
  }

  function saveAndShowSolution(card: Card) {
    return withErrorHandling(setState, async () => {
      await api.updateCard(card, true);
      setRouterState({ route: "Solution", card });
    });
  }

  function deleteAndGroom(searchText: string) {
    return (id: string) => {
      return withErrorHandling(setState, async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
    };
  }

  async function enable(id: string, searchText: string) {
    await withErrorHandling(setState, async () => {
      await api.enable(id);
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  async function disable(id: string, searchText: string) {
    await withErrorHandling(setState, async () => {
      await api.disable(id);
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  async function groomEdit(card: GroomCard, searchText: string) {
    setRouterState({
      route: "GroomEdit",
      card: card,
      searchText: searchText,
    });
  }

  async function saveGroomItem(
    isMinor: boolean,
    searchText: string,
    card: GroomCard,
  ) {
    await withErrorHandling(setState, async () => {
      await api.updateCard(card, isMinor);
      setRouterState({ route: "GroomItem", searchText, card });
    });
  }

  async function backFromGromItem(searchText: string) {
    await withErrorHandling(setState, async () => {
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  function setRouterState(routerState: RouterState) {
    setState((prevState) => ({ ...prevState, routerState }));
  }
};

async function withErrorHandling(
  setState: SetStateType,
  body: () => Promise<void>,
) {
  setState((prevState) => ({ ...prevState, isContactingServer: true }));

  try {
    await body();

    setState((prevState) => ({
      ...prevState,
      serverError: undefined,
      isContactingServer: false,
    }));
  } catch (e) {
    setState((prevState) =>
      e.message === "unauthenticated"
        ? {
            ...prevState,
            serverError: e.message,
            routerState: { route: "Login" },
            isContactingServer: false,
          }
        : {
            ...prevState,
            serverError: e.message,
            isContactingServer: false,
          },
    );
  }
}

async function promptNext(setState: SetStateType) {
  const card = await api.findNextCard();

  setState((prevState: AppState) => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" },
  }));
}
