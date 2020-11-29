export type Card = { id: string; prompt: string; solution: string };
export type CardState = "new" | "ok" | "failed";
export type GroomCard = Card & { disabled: boolean; state: CardState };
export type FindResponseCard = {
  id: string;
  prompt: string;
  disabled: boolean;
};
export type FindResponse = {
  cards: FindResponseCard[];
  count?: number;
  pageCount?: number;
};

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
      page: number;
      findResponse: FindResponse;
    }
  | {
      route: "GroomSingle";
      card: GroomCard;
      searchText: string;
      page: number;
    }
  | {
      route: "GroomEdit";
      card: GroomCard;
      searchText: string;
      page: number;
    }
  | { route: "Recover"; card: Card };

export type AppState = {
  routerState: RouterState;
  isContactingServer: boolean;
  serverError?: Error;
};

export type Api = {
  login: (
    userName: string,
    password: string,
  ) => Promise<{ autoSave: Card | undefined }>;
  createCard: (prompt: string, solution: string) => Promise<void>;
  readCard: (id: string) => Promise<GroomCard | undefined>;
  updateCard: (card: Card) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<void>;
  setFailed: (id: string) => Promise<void>;
  findCards: (searchText: string, page: number) => Promise<FindResponse>;
  enable: (id: string) => Promise<void>;
  disable: (id: string) => Promise<void>;
  deleteHistory: (id: string) => Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAutoSave: () => Promise<void>;
};

export type AppNarratives = {
  login: (userName: string, password: string) => Promise<void>;
  showSolution: (card: Card) => () => Promise<void>;
  goToPrompt: () => Promise<void>;
  setOk: (id: string) => () => Promise<void>;
  setFailed: (id: string) => () => Promise<void>;
  editSolution: (card: Card) => () => Promise<void>;
  saveAndShowSolution(card: Card): Promise<void>;
  cancelEdit: (card: Card) => () => Promise<void>;
  goToCreate: () => Promise<void>;
  goToGroom: () => Promise<void>;
  setCards: (searchText: string) => Promise<void>;
  goToGroomPage: (searchText: string, page: number) => Promise<void>;
  groomSingle: (
    searchText: string,
    page: number,
  ) => (id: string) => Promise<void>;
  groomEdit: (
    card: GroomCard,
    searchText: string,
    page: number,
  ) => () => Promise<void>;
  enable: (searchText: string, page: number) => (id: string) => Promise<void>;
  disable: (searchText: string, page: number) => (id: string) => Promise<void>;
  backFromGroomSingle: (
    searchText: string,
    page: number,
  ) => () => Promise<void>;
  saveFromGroom: (
    searchText: string,
    page: number,
    disabled: boolean,
    state: CardState,
  ) => (card: Card) => Promise<void>;
  cancelGroomEdit: (
    card: GroomCard,
    searchText: string,
    page: number,
  ) => () => Promise<void>;
  delete: (searchText: string, page: number) => (id: string) => Promise<void>;
  deleteHistory: (
    searchText: string,
    page: number,
    id: GroomCard,
  ) => Promise<void>;
  saveRecovered(card: Card): Promise<void>;
  abandonRecovered(): Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
