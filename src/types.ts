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
  onSaveAsNew: (card: Card) => void;
  onSave: (card: Card) => void;
};
export type GroomEditState = {
  route: "GroomEdit";
  card: Card;
  searchText: string;
  onSaveAsNew: (card: Card) => void;
  onCancel: () => void;
  onSave: (card: Card) => void;
};
export type RecoverState = {
  route: "Recover";
  card: Card;
  onAbandon: () => void;
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
  writeAutoSave: (card: Card) => Promise<void>;
  deleteAndGroom: (searchText: string) => (id: string) => Promise<void>;
  deleteAndNext: (id: string) => Promise<void>;
  cancelEdit(card: Card): Promise<void>;
}>;

export type SetStateType = React.Dispatch<React.SetStateAction<AppState>>;
