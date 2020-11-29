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
        const findResponse = await api.findCards("", 0);
        setRouterState({
          route: "Groom",
          findResponse,
          searchText: "",
          page: 0,
        });
      }),

    goToGroom: async () =>
      await handle(async () => {
        setRouterState({
          route: "Groom",
          findResponse: { cards: [] },
          searchText: "",
          page: 0,
        });
      }),

    setCards: async (searchText) =>
      await handle(async () => {
        const page = 0;
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
      }),

    goToGroomPage: async (searchText, page) =>
      await handle(async () => {
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
      }),

    groomSingle: (searchText, page) => async (id) =>
      await handle(async () => {
        const card = await api.readCard(id);
        if (card !== undefined)
          setRouterState({ route: "GroomSingle", card, searchText, page });
      }),

    groomEdit: (card, searchText, page) => async () =>
      setRouterState({ route: "GroomEdit", card, searchText, page }),

    enable: (searchText, page) => async (id) =>
      await handle(async () => {
        await api.enable(id);
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
      }),

    disable: (searchText, page) => async (id) =>
      await handle(async () => {
        await api.disable(id);
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
      }),

    backFromGroomSingle: (searchText, page) => async () =>
      await handle(async () => {
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
      }),

    saveFromGroom: (searchText, page, disabled, state) => async (card) =>
      await handle(async () => {
        await api.updateCard(card);
        setRouterState({
          route: "GroomSingle",
          searchText,
          page,
          card: { ...card, disabled, state },
        });
      }),

    deleteHistory: async (searchText, page, card) =>
      await handle(async () => {
        await api.deleteHistory(card.id);
        setRouterState({
          route: "GroomSingle",
          searchText,
          page,
          card: { ...card, state: "new" },
        });
      }),

    cancelGroomEdit: (card, searchText, page) => async () => {
      setRouterState({ route: "GroomSingle", card, searchText, page });
      await handle(async () => await api.deleteAutoSave());
    },

    delete: (searchText, page) => async (id) =>
      await handle(async () => {
        await api.deleteCard(id);
        const findResponse = await api.findCards(searchText, page);
        setRouterState({ route: "Groom", findResponse, searchText, page });
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
