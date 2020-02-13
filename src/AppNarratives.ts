import { api } from "./Api";
import {
  AppNarratives,
  Card,
  GroomCard,
  GroomItemState,
  GroomState,
  RouterState,
  SolutionState,
  SetStateType,
  AppState,
} from "./types";

export function initialize(setState: SetStateType) {
  return withApi(setState, async () => await promptNext(setState));
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
    saveSnapshot,
  };

  function login(userName: string, password: string) {
    withApi(setState, async () => {
      await api.login(userName, password);
      await promptNext(setState);
    });
  }

  function showSolution(card: Card) {
    setRouterState({ route: "Solution", card });
  }

  function setOk(id: string) {
    withApi(setState, async () => {
      await api.setOk(id);
      await promptNext(setState);
    });
  }

  function setFailed(id: string) {
    withApi(setState, async () => {
      await api.setFailed(id);
      await promptNext(setState);
    });
  }

  function editSolution(prevRouterState: SolutionState) {
    setRouterState({
      route: "Edit",
      card: prevRouterState.card,
      onDelete: deleteAndNext,
      onSaveAsNew: saveAsNewAndNext,
      onCancel: () => {
        setRouterState(prevRouterState);
        withApi(setState, async () => {
          await api.deleteSnapshot();
        });
      },
      onSave: saveAndShowSolution,
    });
  }

  function create(groomState: GroomState) {
    withApi(setState, async () => {
      await api.createCard("New card", "");
      const cards = await api.findCards(groomState.searchText);
      setRouterState({ ...groomState, cards });
    });
  }

  function goToGroom() {
    setRouterState({ route: "Groom", searchText: "", cards: [] });
  }

  function goToPrompt() {
    withApi(setState, async () => {
      await promptNext(setState);
    });
  }

  function setCards(searchText: string) {
    withApi(setState, async () => {
      const cards = await api.findCards(searchText);
      setRouterState({ route: "Groom", cards, searchText });
    });
  }

  function groomItem(prevRouterState: GroomState) {
    return (id: string) =>
      withApi(setState, async () => {
        const card = await api.readCard(id);
        if (card === undefined) {
          return;
        }
        setRouterState(getGroomItemState(card, prevRouterState));
      });
  }

  function saveSnapshot(card: Card) {
    return withApi(setState, async () => {
      await api.saveSnapshot(card);
    });
  }

  function deleteAndNext(id: string) {
    withApi(setState, async () => {
      await api.deleteCard(id);
      await promptNext(setState);
    });
  }

  function saveAsNewAndNext(card: Card) {
    withApi(setState, async () => {
      await api.updateCard(card, false);
      await promptNext(setState);
    });
  }

  function saveAndShowSolution(card: Card) {
    withApi(setState, async () => {
      await api.updateCard(card, true);
      setRouterState({ route: "Solution", card });
    });
  }

  function deleteAndGroom(searchText: string) {
    return (id: string) => {
      withApi(setState, async () => {
        await api.deleteCard(id);
        const cards = await api.findCards(searchText);
        setRouterState({ route: "Groom", cards, searchText });
      });
    };
  }

  function getGroomItemState(card: GroomCard, groomState: GroomState) {
    const groomItemState: GroomItemState = {
      route: "GroomItem",
      card,
      onEnable: (id: string) => {
        withApi(setState, async () => {
          await api.enable(id);
          const cards = await api.findCards(groomState.searchText);
          setRouterState({ ...groomState, cards });
        });
      },
      onDisable: (id: string) => {
        withApi(setState, async () => {
          await api.disable(id);
          const cards = await api.findCards(groomState.searchText);
          setRouterState({ ...groomState, cards });
        });
      },
      onEdit: () =>
        setRouterState({
          route: "Edit",
          card,
          onDelete: deleteAndGroom(groomState.searchText),
          onSaveAsNew: saveGroomItem(false, card.disabled),
          onCancel: () => setRouterState(groomItemState),
          onSave: saveGroomItem(true, card.disabled),
        }),
      onBack: () =>
        withApi(setState, async () => {
          const cards = await api.findCards(groomState.searchText);
          setRouterState({ ...groomState, cards });
        }),
    };
    return groomItemState;

    function saveGroomItem(isMinor: boolean, disabled: boolean) {
      return (cardToSave: Card) => {
        withApi(setState, async () => {
          await api.updateCard(cardToSave, isMinor);
          setRouterState(
            getGroomItemState({ ...cardToSave, disabled }, groomState),
          );
        });
      };
    }
  }

  function setRouterState(routerState: RouterState) {
    setState(prevState => ({ ...prevState, routerState }));
  }
};

async function withApi(setState: SetStateType, apiMethod: () => Promise<void>) {
  setState(prevState => ({ ...prevState, isContactingServer: true }));

  try {
    await apiMethod();

    setState(prevState => ({
      ...prevState,
      serverError: undefined,
      isContactingServer: false,
    }));
  } catch (e) {
    setState(prevState =>
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
