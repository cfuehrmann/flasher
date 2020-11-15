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
        await api.updateCard(card);
        setRouterState({ route: "Solution", card });
      }),

    cancelEdit: (card) => async () => {
      setRouterState({ route: "Solution", card });
      await handle(async () => await api.deleteAutoSave());
    },

    goToCreate: async () =>
      await handle(async () => {
        await api.createCard("New card", "");
        const cards = await api.findCards("");
        setRouterState({ route: "Groom", cards, searchText: "" });
      }),

    goToGroom: async () =>
      await handle(async () => {
        setRouterState({ route: "Groom", cards: [], searchText: "" });
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

    saveFromGroom: (searchText, disabled, state) => async (card) =>
      await handle(async () => {
        await api.updateCard(card);
        setRouterState({
          route: "GroomSingle",
          searchText,
          card: { ...card, disabled, state },
        });
      }),

    deleteHistory: async (searchText, card) =>
      await handle(async () => {
        await api.deleteHistory(card.id);
        setRouterState({
          route: "GroomSingle",
          searchText,
          card: { ...card, state: "new" },
        });
      }),

    cancelGroomEdit: (card, searchText) => async () => {
      setRouterState({ route: "GroomSingle", card, searchText });
      await handle(async () => await api.deleteAutoSave());
    },

    delete: (searchText) => async (id) =>
      await handle(async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      }),

    saveRecovered: async (card) =>
      await handle(async () => {
        await api.updateCard(card);
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
      e.status === 401
        ? {
            ...prevState,
            routerState: { route: "Login" },
            isContactingServer: false,
          }
        : {
            ...prevState,
            serverError: e,
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
