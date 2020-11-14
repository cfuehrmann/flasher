import * as React from "react";

import { TextButton } from "./Buttons";
import { GroomCard } from "./types";

export function GroomView(props: {
  onGoToPrompt: () => void;
  onChangeInput: (searchText: string) => void;
  onGroomItem: (id: string) => void;
  onGoToCreate: () => void;
  searchText: string;
  cards: GroomCard[];
}) {
  return (
    <>
      <div className="w3-bar">
        <button className="w3-bar-item w3-button" onClick={props.onGoToPrompt}>
          Prompt
        </button>
        <input
          className="w3-bar-item w3-input w3-border w3-right"
          type="text"
          autoFocus
          onChange={(event) =>
            props.onChangeInput((event.target as HTMLInputElement).value)
          }
          value={props.searchText}
          placeholder="Search all cards..."
        />
      </div>

      <TextButton key="xxx" text="+" onClick={props.onGoToCreate} />

      {List()}
    </>
  );

  function List() {
    const disabledCards = props.cards.filter((c) => c.disabled);

    const enabledCardButtons = props.cards
      .filter((c) => !c.disabled)
      .map(getCardButton);

    return disabledCards.length === 0 ? (
      enabledCardButtons
    ) : (
      <>
        <p>Disabled</p>
        {disabledCards.map(getCardButton)}
        <p>Enabled</p>
        {enabledCardButtons}
      </>
    );
  }

  function getCardButton(c: GroomCard) {
    return (
      <TextButton
        key={c.id}
        text={c.prompt}
        onClick={() => props.onGroomItem(c.id)}
      />
    );
  }
}
