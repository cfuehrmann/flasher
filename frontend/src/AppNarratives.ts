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

    goToGroom: async () => setRouterState({ route: "Groom" }),

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

    handle,
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
