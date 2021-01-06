import { toast } from "react-toastify";
import { api } from "./Api";
import {
  AppNarratives,
  RouterState,
  SetStateType,
  AppState,
  Card,
} from "./types";

export const initialize = async (setState: SetStateType) =>
  handleWithState(setState, async () => {
    const card = await api.findNextCard();
    setState((prevState: AppState) => ({
      ...prevState,
      routerState: card ? { route: "Prompt", card } : { route: "Done" },
    }));
  })();

export const getNarratives = (setState: SetStateType): AppNarratives => {
  const setPromptOrDone = (card: Card | undefined) =>
    setState((prevState: AppState) => ({
      ...prevState,
      routerState: card ? { route: "Prompt", card } : { route: "Done" },
    }));

  return {
    login: handleApi(async (userName, password) => {
      const { autoSave } = await api.login(userName, password);
      if (autoSave) setRouterState({ route: "Recover", card: autoSave });
      else {
        const card = await api.findNextCard();
        setPromptOrDone(card);
      }
    }),

    showSolution: (card) => async () =>
      setRouterState({ route: "Solution", card }),

    goToPrompt: handleApi(async () => {
      const card = await api.findNextCard();
      setPromptOrDone(card);
    }),

    setOk: (id: string) =>
      handleApi(async () => {
        const card = await api.setOk(id);
        setPromptOrDone(card);
      }),

    setFailed: (id) =>
      handleApi(async () => {
        const card = await api.setFailed(id);
        setPromptOrDone(card);
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
          const nextCard = await api.findNextCard();
          setPromptOrDone(nextCard);
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
          const nextCard = await api.findNextCard();
          setPromptOrDone(nextCard);
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
