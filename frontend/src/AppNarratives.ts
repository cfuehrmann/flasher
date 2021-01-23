import { toast } from "react-toastify";
import { api } from "./Api";
import { AppNarratives, RouterState, SetStateType, AppState } from "./types";

export const initialize = async (setState: SetStateType) =>
  handleWithState(setState, async () => {
    setState((prevState: AppState) => ({
      ...prevState,
      routerState: { route: "Prompt" },
    }));
  })();

export const getNarratives = (setState: SetStateType): AppNarratives => {
  const setPrompt = () =>
    setState((prevState: AppState) => ({
      ...prevState,
      routerState: { route: "Prompt" },
    }));

  return {
    login: handleApi(async (userName, password) => {
      const { autoSave } = await api.login(userName, password);
      if (autoSave) setRouterState({ route: "Recover", card: autoSave });
      else {
        setPrompt();
      }
    }),

    goToPrompt: handleApi(async () => {
      setPrompt();
    }),

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
          setPrompt();
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
          setPrompt();
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
