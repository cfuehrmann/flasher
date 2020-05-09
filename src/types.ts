export type Card = Readonly<{
  id: string;
  prompt: string;
  solution: string;
}>;

export type GroomCard = Readonly<{
  id: string;
  prompt: string;
  solution: string;
  disabled: boolean;
}>;

export type LoginState = {
  route: "Login";
};
export type PromptState = {
  route: "Prompt";
  card: Card;
};
export type SolutionState = {
  route: "Solution";
  card: Card;
};
export type EditState = {
  route: "Edit";
  card: Card;
};
export type GroomEditState = {
  route: "GroomEdit";
  card: GroomCard;
  searchText: string;
};
export type RecoverState = {
  route: "Recover";
  card: Card;
};
export type StartingState = { route: "Starting" };
export type DoneState = { route: "Done" };
export type GroomState = {
  route: "Groom";
  searchText: string;
  cards: GroomCard[];
};
export type GroomItemState = {
  route: "GroomItem";
  card: GroomCard;
  searchText: string;
};

export type RouterState =
  | LoginState
  | PromptState
  | SolutionState
  | EditState
  | GroomEditState
  | RecoverState
  | StartingState
  | DoneState
  | GroomState
  | GroomItemState;

export type AppState = {
  routerState: RouterState;
  isContactingServer: boolean;
  serverError?: unknown;
};

export type Api = Readonly<{
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
}>;

export type AppNarratives = {
  login: (userName: string, password: string) => Promise<void>;
  goToPrompt: () => Promise<void>;
  goToGroom: () => void;
  showSolution: (card: Card) => void;
  setOk: (id: string) => void;
  setFailed: (id: string) => void;
  editSolution: (prevRouterState: SolutionState) => void;
  create: (prevRouterState: GroomState) => void;
  setCards: (searchText: string) => void;
  groomItem: (searchText: string) => (id: string) => void;
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAndGroom: (searchText: string) => (id: string) => Promise<void>;
  deleteAndNext: (id: string) => Promise<void>;
  cancelEdit(card: Card): Promise<void>;
  cancelGroomEdit(card: GroomCard, searchText: string): Promise<void>;
  enable: (searchText: string) => (id: string) => Promise<void>;
  disable: (searchText: string) => (id: string) => Promise<void>;
  groomEdit(card: GroomCard, searchText: string): Promise<void>;
  backFromGromItem(searchText: string): Promise<void>;
  saveGroomItem(
    isMinor: boolean,
    searchText: string,
    card: GroomCard,
  ): Promise<void>;
  saveAsNewAndNext(card: Card): Promise<void>;
  saveAndShowSolution(card: Card): Promise<void>;
  saveRecovered(card: Card): Promise<void>;
  abandonRecovered(): Promise<void>;
};

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
