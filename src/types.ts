export type Card = { id: string; prompt: string; solution: string };

export type GroomCard = Card & { disabled: boolean };

export type RouterState =
  | { route: "Starting" }
  | { route: "Login" }
  | { route: "Prompt"; card: Card }
  | { route: "Done" }
  | { route: "Solution"; card: Card }
  | { route: "Edit"; card: Card }
  | {
      route: "Groom";
      searchText: string;
      cards: GroomCard[];
    }
  | {
      route: "GroomSingle";
      card: GroomCard;
      searchText: string;
    }
  | {
      route: "GroomEdit";
      card: GroomCard;
      searchText: string;
    }
  | { route: "Recover"; card: Card };

export type AppState = {
  routerState: RouterState;
  isContactingServer: boolean;
  serverError?: unknown;
};

export type Api = {
  login: (
    userName: string,
    password: string,
  ) => Promise<{ autoSave: Card | undefined }>;
  createCard: (prompt: string, solution: string) => Promise<void>;
  readCard: (id: string) => Promise<GroomCard | undefined>;
  updateCard: (card: Card, isMinor: boolean) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<void>;
  setFailed: (id: string) => Promise<void>;
  findCards: (substring: string) => Promise<GroomCard[]>;
  enable: (id: string) => Promise<void>;
  disable: (id: string) => Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAutoSave: () => Promise<void>;
};

export type AppNarratives = {
  login: (userName: string, password: string) => Promise<void>;
  goToGroom: () => Promise<void>;
  showSolution: (card: Card) => () => Promise<void>;
  goToPrompt: () => Promise<void>;
  setOk: (id: string) => () => Promise<void>;
  setFailed: (id: string) => () => Promise<void>;
  editSolution: (card: Card) => () => Promise<void>;
  saveAndShowSolution(card: Card): Promise<void>;
  saveAsNewAndNext(card: Card): Promise<void>;
  cancelEdit: (card: Card) => () => Promise<void>;
  deleteAndNext: (id: string) => Promise<void>;
  goToCreate: (searchText: string) => () => Promise<void>;
  setCards: (searchText: string) => Promise<void>;
  groomSingle: (searchText: string) => (id: string) => Promise<void>;
  groomEdit: (card: GroomCard, searchText: string) => () => Promise<void>;
  enable: (searchText: string) => (id: string) => Promise<void>;
  disable: (searchText: string) => (id: string) => Promise<void>;
  backFromGroomSingle: (searchText: string) => () => Promise<void>;
  saveFromGroom: (
    isMinor: boolean,
    searchText: string,
    disabled: boolean,
  ) => (card: Card) => Promise<void>;
  cancelGroomEdit: (card: GroomCard, searchText: string) => () => Promise<void>;
  deleteAndGroom: (searchText: string) => (id: string) => Promise<void>;
  saveRecovered(card: Card): Promise<void>;
  abandonRecovered(): Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
