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
    saveRecovered,
    abandonRecovered,
  };

  async function login(userName: string, password: string) {
    await withErrorHandling(setState, async () => {
      const { autoSave } = await api.login(userName, password);

      if (autoSave) {
        setRouterState({
          route: "Recover",
          card: autoSave,
        });
      } else await promptNext(setState);
    });
  }

  async function saveRecovered(card: Card) {
    await withErrorHandling(setState, async () => {
      await api.updateCard(card, true);
      await promptNext(setState);
    });
  }

  async function abandonRecovered() {
    await withErrorHandling(setState, async () => {
      await api.deleteAutoSave();
      await promptNext(setState);
    });
  }

  async function goToPrompt() {
    await withErrorHandling(setState, async () => {
      await promptNext(setState);
    });
  }

  async function goToGroom() {
    setRouterState({ route: "Groom", searchText: "", cards: [] });
  }

  async function showSolution(card: Card) {
    setRouterState({ route: "Solution", card });
  }

  async function setOk(id: string) {
    await withErrorHandling(setState, async () => {
      await api.setOk(id);
      await promptNext(setState);
    });
  }

  async function setFailed(id: string) {
    await withErrorHandling(setState, async () => {
      await api.setFailed(id);
      await promptNext(setState);
    });
  }

  async function editSolution(prevRouterState: SolutionState) {
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

  async function create(groomState: GroomState) {
    await withErrorHandling(setState, async () => {
      await api.createCard("New card", "");
      const cards = await api.findCards(groomState.searchText);
      setRouterState({ ...groomState, cards });
    });
  }

  async function setCards(searchText: string) {
    await withErrorHandling(setState, async () => {
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  function groomItem(searchText: string) {
    return async (id: string) =>
      await withErrorHandling(setState, async () => {
        const card = await api.readCard(id);
        if (card !== undefined) {
          setRouterState({
            route: "GroomItem",
            card,
            searchText,
          });
        }
      });
  }

  async function writeAutoSave(card: Card) {
    await withErrorHandling(setState, async () => {
      await api.writeAutoSave(card);
    });
  }

  async function deleteAndNext(id: string) {
    await withErrorHandling(setState, async () => {
      await api.deleteCard(id);
      await promptNext(setState);
    });
  }

  async function saveAsNewAndNext(card: Card) {
    await withErrorHandling(setState, async () => {
      await api.updateCard(card, false);
      await promptNext(setState);
    });
  }

  async function saveAndShowSolution(card: Card) {
    await withErrorHandling(setState, async () => {
      await api.updateCard(card, true);
      setRouterState({ route: "Solution", card });
    });
  }

  function deleteAndGroom(searchText: string) {
    return async (id: string) => {
      await withErrorHandling(setState, async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
    };
  }

  function enable(searchText: string) {
    return async (id: string) =>
      await withErrorHandling(setState, async () => {
        await api.enable(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
  }

  function disable(searchText: string) {
    return async (id: string) =>
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
