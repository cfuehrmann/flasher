import { Api, Card, GroomCard } from "./types";

export const api: Api = {
  login: async (userName: string, password: string) => {
    await postAsJson({
      query: `query login($userName: String!, $password: String!) {
      login(userName: $userName, password: $password)
    }`,
      variables: {
        userName,
        password,
      },
    });
  },
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
          disabled
        }
      }`,
      variables: {
        id,
      },
    });

    return (data as { readCard: GroomCard | undefined }).readCard;
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
          disabled
        }
      }`,
      variables: {
        substring,
      },
    });
    return (data as { cards: GroomCard[] }).cards;
  },

  enable: async (id: string) => {
    await postAsJson({
      query: `mutation enable($id: ID!) {
        enable(id: $id)
      }`,
      variables: { id: id },
    });
  },

  disable: async (id: string) => {
    await postAsJson({
      query: `mutation disable($id: ID!) {
        disable(id: $id)
      }`,
      variables: { id: id },
    });
  },
};

async function postAsJson(body: {
  query: string;
  variables?: {};
}): Promise<{}> {
  const fetchResponse = await fetch(
    window.location.protocol +
      "//" +
      window.location.hostname +
      "/flasher_api/graphql",
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(body),
    },
  );

  const responseObject: {
    data: {};
    errors?: Array<{ message: string }>;
  } = await fetchResponse.json();

  if (responseObject.errors) {
    throw new Error(responseObject.errors.map(e => e.message).join("\n"));
  }

  return responseObject.data;
}
