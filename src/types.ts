export type Card = Readonly<{ id: string; prompt: string; solution: string }>;

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
export type CreateState = {
  route: "Create";
  prompt: string;
  solution: string;
  onCancel: () => void;
  onSave: (prompt: string, solution: string) => void;
};
export type CheckCreatedState = {
  route: "CheckCreated";
  prompt: string;
  solution: string;
  onCancel: () => void;
  onEdit: () => void;
  onSave: () => void;
};
export type GroomState = { route: "Groom"; searchText: string; cards: Card[] };
export type GroomItemState = {
  route: "GroomItem";
  card: Card;
  onBack: () => void;
  onEdit: () => void;
};

export type RouterState =
  | PromptState
  | SolutionState
  | EditState
  | StartingState
  | DoneState
  | CreateState
  | GroomState
  | GroomItemState
  | CheckCreatedState;

export type AppState = {
  routerState: RouterState;
  isFetching: boolean;
  apiError?: unknown;
};

export type Api = Readonly<{
  createCard: (prompt: string, solution: string) => Promise<void>;
  readCard: (id: string) => Promise<Card | undefined>;
  updateCard: (card: Card, isMinor: boolean) => Promise<Card>;
  deleteCard: (id: string) => Promise<void>;
  findNextCard: () => Promise<Card | undefined>;
  setOk: (id: string) => Promise<void>;
  setFailed: (id: string) => Promise<void>;
  findCards: (substring: string) => Promise<Card[]>;
}>;

export type AppNarratives = Readonly<{
  showSolution: (card: Card) => void;
  setOk: (id: string) => void;
  setFailed: (id: string) => void;
  editSolution: (prevRouterState: SolutionState) => void;
  createFromPrompt: (prevRouterState: RouterState) => void;
  createFromGroom: (prevRouterState: GroomState) => void;
  goToGroom: () => void;
  goToPrompt: () => void;
  setCards: (searchText: string) => void;
  groomItem: (prevRouterState: GroomState) => (id: string) => void;
}>;
