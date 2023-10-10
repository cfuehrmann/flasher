import { toast } from "react-toastify";
import { api } from "./Api";
import { AppNarratives, AppState, RouterState, SetStateType } from "./types";

export const getNarratives = (setState: SetStateType): AppNarratives => {
  const setPrompt = () =>
    setState(
      (prevState: AppState): AppState => ({
        ...prevState,
        routerState: { route: "Prompt" },
      }),
    );

  return {
    login: handleApi(async (userName, password) => {
      const { autoSave } = await api.login(userName, password);

      if (autoSave) {
        setRouterState({ route: "Recover", card: autoSave });
      } else {
        setPrompt();
      }
    }),

    goToPrompt: setPrompt,

    goToGroom: () => setRouterState({ route: "Groom" }),

    handleApi,
  };

  function setRouterState(routerState: RouterState) {
    setState(
      (prevState: AppState): AppState => ({ ...prevState, routerState }),
    );
  }

  function handleApi<T extends readonly unknown[]>(
    body: (...args: T) => Promise<void>,
  ) {
    {
      return async (...args: T) => {
        setState(
          (prevState: AppState): AppState => ({
            ...prevState,
            isContactingServer: true,
          }),
        );

        try {
          await body(...args);

          setState(
            (prevState: AppState): AppState => ({
              ...prevState,
              isContactingServer: false,
            }),
          );
        } catch (e) {
          if (
            typeof e === "object" &&
            e !== null &&
            "status" in e &&
            e.status === 401
          ) {
            setState(
              (prevState: AppState): AppState => ({
                ...prevState,
                routerState: { route: "Login" },
                isContactingServer: false,
              }),
            );
          } else {
            const message = getErrorMessage(e);

            toast(message, {
              type: "error",
              position: "bottom-right",
            });

            setState(
              (prevState: AppState): AppState => ({
                ...prevState,
                isContactingServer: false,
              }),
            );
          }
        }
      };
    }
  }
};

function getErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = error.message;

    if (typeof message === "string") {
      return message;
    }
  }
  return "Unknown server error!";
}
