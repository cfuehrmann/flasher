import { api } from "./Api";
import { AppNarratives, RouterState, SetStateType, AppState } from "./types";

export const initialize = async (setState: SetStateType) =>
  await handleWithState(
    setState,
    async () => await promptNextWithState(setState),
  );

export const getNarratives = (setState: SetStateType): AppNarratives => {
  return {
    login: async (userName, password) =>
      await handle(async () => {
        const { autoSave } = await api.login(userName, password);

        if (autoSave) setRouterState({ route: "Recover", card: autoSave });
        else await promptNext();
      }),

    goToGroom: async () =>
      setRouterState({ route: "Groom", searchText: "", cards: [] }),

    showSolution: (card) => async () =>
      setRouterState({ route: "Solution", card }),

    goToPrompt: async () => await handle(async () => await promptNext()),

    setOk: (id) => async () =>
      await handle(async () => {
        await api.setOk(id);
        await promptNext();
      }),

    setFailed: (id) => async () =>
      await handle(async () => {
        await api.setFailed(id);
        await promptNext();
      }),

    editSolution: (card) => async () => setRouterState({ route: "Edit", card }),

    saveAndShowSolution: async (card) =>
      await handle(async () => {
        await api.updateCard(card, true);
        setRouterState({ route: "Solution", card });
      }),

    saveAsNewAndNext: async (card) =>
      await handle(async () => {
        await api.updateCard(card, false);
        await promptNext();
      }),

    cancelEdit: (card) => async () => {
      setRouterState({ route: "Solution", card });
      await handle(async () => await api.deleteAutoSave());
    },

    deleteAndNext: async (id) =>
      await handle(async () => {
        await api.deleteCard(id);
        await promptNext();
      }),

    goToCreate: (searchText) => async () =>
      await handle(async () => {
        await api.createCard("New card", "");
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    setCards: async (searchText) =>
      await handle(async () => {
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    groomSingle: (searchText) => async (id) =>
      await handle(async () => {
        const card = await api.readCard(id);
        if (card !== undefined)
          setRouterState({ route: "GroomSingle", card, searchText });
      }),

    groomEdit: (card, searchText) => async () =>
      setRouterState({ route: "GroomEdit", card, searchText }),

    enable: (searchText) => async (id) =>
      await handle(async () => {
        await api.enable(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    disable: (searchText) => async (id) =>
      await handle(async () => {
        await api.disable(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    backFromGroomSingle: (searchText) => async () =>
      await handle(async () => {
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    saveFromGroom: (isMinor, searchText, disabled) => async (card) =>
      await handle(async () => {
        await api.updateCard(card, isMinor);
        setRouterState({
          route: "GroomSingle",
          searchText,
          card: { ...card, disabled },
        });
      }),

    cancelGroomEdit: (card, searchText) => async () => {
      setRouterState({ route: "GroomSingle", card, searchText });
      await handle(async () => await api.deleteAutoSave());
    },

    deleteAndGroom: (searchText) => async (id) =>
      await handle(async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    saveRecovered: async (card) =>
      await handle(async () => {
        await api.updateCard(card, true);
        await promptNext();
      }),

    abandonRecovered: async () =>
      await handle(async () => {
        await api.deleteAutoSave();
        await promptNext();
      }),

    writeAutoSave: async (card) =>
      await handle(async () => await api.writeAutoSave(card)),
  };

  function setRouterState(routerState: RouterState) {
    setState((prevState) => ({ ...prevState, routerState }));
  }

  async function handle(body: () => Promise<void>) {
    await handleWithState(setState, body);
  }

  async function promptNext() {
    await promptNextWithState(setState);
  }
};

async function handleWithState(
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

async function promptNextWithState(setState: SetStateType) {
  const card = await api.findNextCard();

  setState((prevState: AppState) => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" },
  }));
}
