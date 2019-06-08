import * as React from "preact";

import { TextButton } from "./Buttons";
import { Card } from "./types";

export function GroomView(props: {
  onGoToPrompt: () => void;
  onChangeInput: (searchText: string) => void;
  onGroomItem: (id: string) => void;
  onGoToCreate: () => void;
  searchText: string;
  cards: Card[];
}) {
  return (
    <>
      <div className="w3-bar">
        <button className="w3-bar-item w3-button" onClick={props.onGoToCreate}>
          Create
        </button>
        <button className="w3-bar-item w3-button" onClick={props.onGoToPrompt}>
          Prompt
        </button>
        <input
          className="w3-bar-item w3-input"
          type="text"
          onChange={event =>
            props.onChangeInput((event.target as HTMLInputElement).value)
          }
          value={props.searchText}
          placeholder="Search.."
        />
      </div>
      <List />
    </>
  );

  function List() {
    return (
      <>
        {props.cards.map(c => (
          <TextButton
            key={c.id}
            text={c.prompt}
            onClick={() => props.onGroomItem(c.id)}
          />
        ))}
      </>
    );
  }
}
