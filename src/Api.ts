import { apiUrl } from "./ApiConfig.js";
import { Api, Card } from "./types";

export const api: Api = {
  findNextCard: async () => {
    const data = await postAsJson({
      query: `query findNextCard {
        findNextCard {
            id
            prompt
            solution
        }
      }`,
    });

    return (data as { findNextCard: Card | undefined }).findNextCard;
  },

  setOk: async (id: string) => {
    await postAsJson({
      query: `mutation setOk($id: ID!) {
        setOk(id: $id)
      }`,
      variables: { id: id },
    });
  },

  setFailed: async (id: string) => {
    await postAsJson({
      query: `mutation setFailed($id: ID!) {
        setFailed(id: $id)
      }`,
      variables: { id: id },
    });
  },

  createCard: async (prompt, solution) => {
    await postAsJson({
      query: `mutation createCard($prompt: String!, $solution: String!) {
        createCard(prompt: $prompt, solution: $solution)
      }`,
      variables: {
        prompt,
        solution,
      },
    });
  },

  readCard: async (id: string) => {
    const data = await postAsJson({
      query: `query readCard($id: ID!) {
        readCard(id: $id) {
          id
          prompt
          solution
        }
      }`,
      variables: {
        id,
      },
    });

    return (data as { readCard: Card | undefined }).readCard;
  },

  updateCard: async (card, isMinor) => {
    const data = await postAsJson({
      query: `mutation updateCard($id: ID!, $prompt: String!, $solution: String!, $isMinor: Boolean!) {
              updateCard(id: $id, prompt: $prompt, solution: $solution, isMinor: $isMinor) {
                id
                prompt
                solution
              }
            }`,
      variables: {
        ...card,
        isMinor,
      },
    });
    return (data as { updateCard: Card }).updateCard;
  },

  deleteCard: async id => {
    await postAsJson({
      query: `mutation deleteCard($id: ID!) {
        deleteCard(id: $id) 
      }`,
      variables: {
        id,
      },
    });
  },

  findCards: async (substring: string) => {
    const data = await postAsJson({
      query: `query cards($substring: String!) {
        cards(substring: $substring) {
          id
          prompt
        }
      }`,
      variables: {
        substring,
      },
    });
    return (data as { cards: Card[] }).cards;
  },
};

async function postAsJson(body: {
  query: string;
  variables?: {};
}): Promise<{}> {
  const fetchResponse = await fetch(apiUrl, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(body),
  });

  const responseObject: {
    data: {};
    errors?: Array<{ message: string }>;
  } = await fetchResponse.json();

  if (responseObject.errors) {
    throw new Error(responseObject.errors.map(e => e.message).join("\n"));
  }

  return responseObject.data;
}
