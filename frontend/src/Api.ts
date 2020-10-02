import { Api, Card } from "./types";

export const api: Api = {
  login: async (userName: string, password: string) => {
    const response = await post(`Authentication/Login`, { userName, password });
    return await response.json();
  },

  findNextCard: async () => {
    const response = await get("Cards/Next");

    return response.status === 200 ? await response.json() : undefined;
  },

  setOk: async (id: string) => {
    await post(`Cards/${id}/SetOk`);
  },

  setFailed: async (id: string) => {
    await post(`Cards/${id}/SetFailed`);
  },

  createCard: async (prompt, solution) => {
    await post(`Cards`, { prompt, solution });
  },

  readCard: async (id: string) => {
    const response = await get(`Cards/${id}`);
    return await response.json();
  },

  updateCard: async (card) => {
    await patch(`Cards/${card.id}`, card);
    return { id: "dummy", prompt: "dummy", solution: "dummy" };
  },

  deleteCard: async (id) => await del(`Cards/${id}`),

  findCards: async (searchText: string) => {
    const response = await get(`Cards?searchText=${searchText}`);
    const json = await response.json();
    return json.cards;
  },

  enable: async (id: string) => {
    await post(`Cards/${id}/Enable`);
  },

  disable: async (id: string) => {
    await post(`Cards/${id}/Disable`);
  },

  deleteHistory: async (id) => await del(`History/${id}`),

  writeAutoSave: async (card: Card) => await put(`AutoSave`, card),

  deleteAutoSave: async () => await del("AutoSave"),
};

async function get(url: string) {
  return await sendRequest("GET", url);
}

async function post(url: string, body?: {}) {
  return await sendRequest("POST", url, body);
}

async function patch(url: string, body: {}) {
  await sendRequest("PATCH", url, body);
}

async function put(url: string, body: {}) {
  await sendRequest("PUT", url, body);
}

async function del(url: string) {
  await sendRequest("DELETE", url);
}

async function sendRequest(method: string, url: string, body?: {}) {
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
