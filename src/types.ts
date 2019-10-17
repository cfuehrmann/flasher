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
  onDelete: (id: string) => void;
  onSaveAsNew: (card: Card) => void;
  onCancel: () => void;
  onSave: (card: Card) => void;
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
  onEnable: (id: string) => void;
  onDisable: (id: string) => void;
  onBack: () => void;
  onEdit: () => void;
};

export type RouterState =
  | LoginState
  | PromptState
  | SolutionState
  | EditState
  | StartingState
  | DoneState
  | GroomState
  | GroomItemState;

export type AppState = {
  routerState: RouterState;
  isFetching: boolean;
  apiError?: unknown;
};

export type Api = Readonly<{
  login: (userName: string, password: string) => Promise<void>;
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
}>;

export type AppNarratives = Readonly<{
  login: (userName: string, password: string) => void;
  showSolution: (card: Card) => void;
  setOk: (id: string) => void;
  setFailed: (id: string) => void;
  editSolution: (prevRouterState: SolutionState) => void;
  create: (prevRouterState: GroomState) => void;
  goToGroom: () => void;
  goToPrompt: () => void;
  setCards: (searchText: string) => void;
  groomItem: (prevRouterState: GroomState) => (id: string) => void;
}>;

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
