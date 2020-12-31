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
  count: number;
};

export type RouterState =
  | { route: "Starting" }
  | { route: "Login" }
  | { route: "Prompt"; card: Card }
  | { route: "Done" }
  | { route: "Solution"; card: Card }
  | { route: "Edit"; card: Card }
  | { route: "Groom" }
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
  createCard: (prompt: string, solution: string) => Promise<FindResponseCard>;
  readCard: (id: string) => Promise<GroomCard | undefined>;
  updateCard: (card: Card) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<void>;
  setFailed: (id: string) => Promise<void>;
  findCards: (searchText: string, skip: number) => Promise<FindResponse>;
  enable: (id: string) => Promise<void>;
  disable: (id: string) => Promise<void>;
  deleteHistory: (id: string) => Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAutoSave: () => Promise<void>;
};

export type ApiHandler = {
  handleApi: <T extends readonly unknown[]>(
    body: (...args: T) => Promise<void>,
  ) => (...args: T) => Promise<void>;
};

export type AppNarratives = ApiHandler & {
  login: (userName: string, password: string) => Promise<void>;
  showSolution: (card: Card) => () => Promise<void>;
  goToPrompt: () => Promise<void>;
  setOk: (id: string) => () => Promise<void>;
  setFailed: (id: string) => () => Promise<void>;
  editSolution: (card: Card) => () => Promise<void>;
  saveAndShowSolution(card: Card): Promise<void>;
  cancelEdit: (card: Card) => () => Promise<void>;
  goToGroom: () => Promise<void>;
  saveRecovered(card: Card): Promise<void>;
  abandonRecovered(): Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
