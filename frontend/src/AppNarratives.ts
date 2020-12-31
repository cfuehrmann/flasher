import { api } from "./Api";
import { AppNarratives, RouterState, SetStateType, AppState } from "./types";

export const initialize = async (setState: SetStateType) =>
  handleWithState(setState, async () => await promptNextWithState(setState))();

export const getNarratives = (setState: SetStateType): AppNarratives => {
  return {
    login: handleApi(async (userName, password) => {
      const { autoSave } = await api.login(userName, password);
      if (autoSave) setRouterState({ route: "Recover", card: autoSave });
      else await promptNext();
    }),

    showSolution: (card) => async () =>
      setRouterState({ route: "Solution", card }),

    goToPrompt: handleApi(promptNext),

    setOk: (id: string) =>
      handleApi(async () => {
        await api.setOk(id);
        await promptNext();
      }),

    setFailed: (id) =>
      handleApi(async () => {
        await api.setFailed(id);
        await promptNext();
      }),

    editSolution: (card) => async () => setRouterState({ route: "Edit", card }),

    saveAndShowSolution: handleApi(async (card) => {
      await api.updateCard(card);
      setRouterState({ route: "Solution", card });
    }),

    cancelEdit: (card) =>
      handleApi(async () => {
        setRouterState({ route: "Solution", card });
        await api.deleteAutoSave();
      }),

    goToGroom: async () => setRouterState({ route: "Groom" }),

    saveRecovered: handleApi(async (card) => {
      await api.updateCard(card);
      await promptNext();
    }),

    abandonRecovered: handleApi(async () => {
      await api.deleteAutoSave();
      await promptNext();
    }),

    writeAutoSave: handleApi(api.writeAutoSave),

    handleApi,
  };

  function setRouterState(routerState: RouterState) {
    setState((prevState) => ({ ...prevState, routerState }));
  }

  function handleApi<T extends readonly unknown[]>(
    body: (...args: T) => Promise<void>,
  ) {
    return handleWithState(setState, body);
  }

  async function promptNext() {
    await promptNextWithState(setState);
  }
};

function handleWithState<T extends readonly unknown[]>(
  setState: SetStateType,
  body: (...args: T) => Promise<void>,
) {
  return async (...args: T) => {
    setState((prevState) => ({ ...prevState, isContactingServer: true }));

    try {
      await body(...args);

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
  };
}

async function promptNextWithState(setState: SetStateType) {
  const card = await api.findNextCard();

  setState((prevState: AppState) => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" },
  }));
}
