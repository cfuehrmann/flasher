import { toast } from "react-toastify";
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

    saveAndShowSolution: async (
      card,
      clearAutoSaveInterval,
      startAutoSaveInterval,
    ) => {
      clearAutoSaveInterval();
      handleApi(async () => {
        try {
          await api.updateCard(card);
          setRouterState({ route: "Solution", card });
        } catch (_) {
          startAutoSaveInterval();
        }
      })();
    },

    cancelEdit: async (card, clearAutoSaveInterval, startAutoSaveInterval) => {
      clearAutoSaveInterval();
      handleApi(api.deleteAutoSave)();
      setRouterState({ route: "Solution", card });
    },

    goToGroom: async () => setRouterState({ route: "Groom" }),

    saveRecovered: async (
      card,
      clearAutoSaveInterval,
      startAutoSaveInterval,
    ) => {
      clearAutoSaveInterval();
      handleApi(async () => {
        try {
          await api.updateCard(card);
          await promptNext();
        } catch (_) {
          startAutoSaveInterval();
        }
      })();
    },

    abandonRecovered: async (clearAutoSaveInterval, startAutoSaveInterval) => {
      clearAutoSaveInterval();
      handleApi(async () => {
        try {
          await api.deleteAutoSave();
          await promptNext();
        } catch (_) {
          startAutoSaveInterval();
        }
      })();
    },

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
      if (e.status === 401) {
        setState((prevState) => ({
          ...prevState,
          routerState: { route: "Login" },
          isContactingServer: false,
        }));
      } else {
        showServerError(e);
        setState((prevState) => ({
          ...prevState,
          serverError: e,
          isContactingServer: false,
        }));
      }
    }
  };
}

function showServerError(serverError: Error) {
  // toast.configure();
  toast(
    typeof serverError.message === "string"
      ? serverError.message
      : "Unknown server error!",
    { type: "error", position: "bottom-right" },
  );
}

async function promptNextWithState(setState: SetStateType) {
  const card = await api.findNextCard();

  setState((prevState: AppState) => ({
    ...prevState,
    routerState: card ? { route: "Prompt", card } : { route: "Done" },
  }));
}
