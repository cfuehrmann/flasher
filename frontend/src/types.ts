export type Card = { id: string; prompt: string; solution: string };
export type CardState = "new" | "ok" | "failed";
export type GroomCard = Card & { disabled: boolean; state: CardState };
export type FindResponse = { cards: GroomCard[]; count: number };

export type RouterState =
  | { route: "Starting" }
  | { route: "Login" }
  | { route: "Prompt" }
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
  createCard: (prompt: string, solution: string) => Promise<GroomCard>;
  readCard: (id: string) => Promise<GroomCard | undefined>;
  updateCard: (card: Card) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<GroomCard | undefined>;
  setFailed: (id: string) => Promise<GroomCard | undefined>;
  findCards: (searchText: string, skip: number) => Promise<FindResponse>;
  enable: (id: string) => Promise<void>;
  disable: (id: string) => Promise<void>;
  deleteHistory: (id: string) => Promise<GroomCard>;
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
  goToPrompt: () => Promise<void>;
  goToGroom: () => Promise<void>;
  saveRecovered(
    card: Card,
    clearAutoSaveInterval: () => void,
    startAutoSaveInterval: () => void,
  ): Promise<void>;
  abandonRecovered(
    clearAutoSaveInterval: () => void,
    startAutoSaveInterval: () => void,
  ): Promise<void>;
  writeAutoSave: (card: Card) => Promise<void>;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
