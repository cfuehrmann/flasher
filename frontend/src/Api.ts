import { Api, Card, FindResponse, GroomCard } from "./types";

export const api: Api = {
  login: async (userName: string, password: string) => {
    const response = await post(`Authentication/Login`, { userName, password });
    const json: unknown = await response.json();

    const autoSave =
      typeof json === "object" && json !== null && "autoSave" in json
        ? toCard(json.autoSave)
        : undefined;

    return { autoSave };
  },

  findNextCard: async () => {
    const response = await get("Cards/Next");

    if (response.ok) {
      const json: unknown = await response.json();
      return toCard(json);
    }
  },

  setOk: async (id: string) => {
    const response = await post(`Cards/${id}/SetOk`);

    if (response.ok) {
      const json: unknown = await response.json();
      return toCard(json);
    }
  },

  setFailed: async (id: string) => {
    const response = await post(`Cards/${id}/SetFailed`);

    if (response.ok) {
      const json: unknown = await response.json();
      return toCard(json);
    }
  },

  createCard: async (prompt, solution) => {
    const response = await post(`Cards`, { prompt, solution });

    if (response.ok) {
      const json: unknown = await response.json();
      return toGroomCard(json);
    }

    throw new Error("Failed to create card!");
  },

  updateCard: async (card): Promise<Card> => {
    await patch(`Cards/${card.id}`, card);
    return { id: "dummy", prompt: "dummy", solution: "dummy" };
  },

  deleteCard: async (id) => {
    await del(`Cards/${id}`);
  },

  findCards: async (
    searchText: string,
    skip: number,
  ): Promise<FindResponse> => {
    const response = await get(`Cards?searchText=${searchText}&skip=${skip}`);
    const json: unknown = await response.json();
    if (
      typeof json === "object" &&
      json !== null &&
      "cards" in json &&
      "count" in json
    ) {
      const cards = json.cards;
      const count = json.count;

      if (Array.isArray(cards) && typeof count === "number") {
        const groomCards = cards.map(toGroomCard);
        return { cards: groomCards, count };
      }
    }

    throw new Error("Invalid FindResponse");
  },

  enable: async (id: string) => {
    await post(`Cards/${id}/Enable`);
  },

  disable: async (id: string) => {
    await post(`Cards/${id}/Disable`);
  },

  deleteHistory: async (id) => {
    const response = await del(`History/${id}`);

    if (response.ok) {
      const json: unknown = await response.json();
      return toGroomCard(json);
    }

    throw new Error("Did not find card from which to delete history!");
  },

  writeAutoSave: async (card: Card) => {
    await put(`AutoSave`, card);
  },

  deleteAutoSave: async () => {
    await del("AutoSave");
  },
};

async function get(url: string) {
  return await sendRequest("GET", url);
}

async function post(url: string, body?: unknown) {
  return await sendRequest("POST", url, body);
}

async function patch(url: string, body: unknown) {
  await sendRequest("PATCH", url, body);
}

async function put(url: string, body: unknown) {
  await sendRequest("PUT", url, body);
}

async function del(url: string) {
  return await sendRequest("DELETE", url);
}

async function sendRequest(method: string, url: string, body?: unknown) {
  const [bodyString, headers] = body
    ? [JSON.stringify(body), { "Content-Type": "application/json" }]
    : [undefined, undefined];

  const response = await fetch(
    window.location.protocol +
      "//" +
      window.location.hostname +
      "/flasher_api/" +
      url,
    { method, body: bodyString, headers },
  );

  if (!response.ok)
    throw Object.assign(new Error(await response.text()), {
      status: response.status,
    });

  return response;
}

function toCard(value: unknown): Card | undefined {
  if (
    typeof value === "object" &&
    value !== null &&
    "id" in value &&
    "prompt" in value &&
    "solution" in value
  ) {
    const id = value.id;
    const prompt = value.prompt;
    const solution = value.solution;

    if (
      typeof id === "string" &&
      typeof prompt === "string" &&
      typeof solution === "string"
    )
      return { id, prompt, solution };
  }
}

function toGroomCard(value: unknown): GroomCard {
  if (
    typeof value === "object" &&
    value !== null &&
    "id" in value &&
    "prompt" in value &&
    "solution" in value &&
    "disabled" in value &&
    "state" in value
  ) {
    const id = value.id;
    const prompt = value.prompt;
    const solution = value.solution;
    const disabled = value.disabled;
    const state = value.state;

    if (
      typeof id === "string" &&
      typeof prompt === "string" &&
      typeof solution === "string" &&
      typeof disabled === "boolean" &&
      (state === "new" || state === "ok" || state === "failed")
    )
      return { id, prompt, solution, disabled, state };
  }

  throw new Error("Invalid GroomCard");
}
