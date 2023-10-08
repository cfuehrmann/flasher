export type Card = {
  id: string;
  prompt: string;
  solution: string;
};
export type CardState = "new" | "ok" | "failed";
export type GroomCard = Card & { disabled: boolean; state: CardState };
export interface FindResponse {
  cards: GroomCard[];
  count: number;
}

export type RouterState =
  | { route: "Starting" }
  | { route: "Login" }
  | { route: "Prompt" }
  | { route: "Groom" }
  | { route: "Recover"; card: Card };

export interface AppState {
  routerState: RouterState;
  isContactingServer: boolean;
}

export interface Api {
  login: (
    userName: string,
    password: string,
  ) => Promise<{ autoSave: Card | undefined }>;
  createCard: (prompt: string, solution: string) => Promise<GroomCard>;
  updateCard: (card: Card) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<Card | undefined>;
  setFailed: (id: string) => Promise<Card | undefined>;
  findCards: (searchText: string, skip: number) => Promise<FindResponse>;
  enable: (id: string) => Promise<void>;
  disable: (id: string) => Promise<void>;
  deleteHistory: (id: string) => Promise<GroomCard>;
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAutoSave: () => Promise<void>;
}

export interface ApiHandler {
  handleApi: <T extends readonly unknown[]>(
    body: (...args: T) => Promise<void>,
  ) => (...args: T) => Promise<void>;
}

export type AppNarratives = ApiHandler & {
  login: (userName: string, password: string) => Promise<void>;
  goToPrompt: () => void;
  goToGroom: () => void;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
