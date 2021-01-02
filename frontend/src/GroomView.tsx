import * as React from "react";
import { useState } from "react";

import { api } from "./Api";
import { ApiHandler, GroomCard } from "./types";
import { TextButton } from "./Buttons";
import { GroomItemView } from "./GroomItemView";
import { EditView } from "./EditView";

export function GroomView(props: ApiHandler & { onGoToPrompt: () => void }) {
  const [modal, setModal] = useState({ kind: "none" } as
    | { kind: "none" }
    | { kind: "view"; card: GroomCard }
    | { kind: "edit"; card: GroomCard });
  const [searchText, setSearchText] = useState("");
  const [activeSearchText, setActiveSearchText] = useState<string | undefined>(
    undefined,
  );
  const [count, setCount] = useState<number | undefined>(undefined);
  const [cards, setCards] = useState<GroomCard[]>([]);

  const handleApi = props.handleApi;

  return (
    <>
      {StickyTop()}
      {Cards()}
      {Modal()}
    </>
  );

  function StickyTop() {
    return (
      <>
        <div className="w3-top">
          <div className="w3-bar w3-light-grey w3-border">
            <TextButton text="Prompt" onClick={props.onGoToPrompt} />
            <TextButton
              text="Create"
              onClick={handleApi(async () => {
                const response = await api.createCard("New card", "");
                setCards([response, ...cards]);
              })}
            />
          </div>
          <div className="w3-bar w3-light-grey w3-border">
            <input
              className="w3-bar-item w3-input"
              type="text"
              autoFocus
              onChange={(event) => setSearchText(event.target.value)}
              value={searchText}
              placeholder="Search..."
            />
            <TextButton
              text="Go"
              onClick={handleApi(async () => {
                const { cards, count } = await api.findCards(searchText, 0);
                setActiveSearchText(searchText);
                setCards(cards);
                setCount(count);
              })}
            />
          </div>
          {activeSearchText !== undefined ? (
            <div className="w3-bar w3-light-grey w3-border">
              <div className="w3-bar-item">{`Showing results for "${activeSearchText}".`}</div>
            </div>
          ) : undefined}
        </div>
        <br />
        <br />
        <br />
        <br />
        <br />
        <br />
      </>
    );
  }

  function Cards() {
    return (
      <div className="w3-container">
        {cards.map(CardButton)}
        <br />
        {count !== undefined ? `Found ${count} cards.` : ""}
        {MoreButton()}
      </div>
    );
  }

  function Modal() {
    switch (modal.kind) {
      case "none":
        return "";
      case "view":
        return ViewModal(modal.card);
      case "edit":
        return EditModal(modal.card);
    }
  }

  function CardButton(card: GroomCard) {
    return (
      <div
        className="w3-card w3-hover-shadow w3-center"
        key={card.id}
        onClick={handleApi(async () => {
          setModal({ kind: "view", card });
        })}
      >
        <div className="w3-container">
          <p>{(card.disabled ? "(Disabled) " : "") + card.prompt}</p>
        </div>
      </div>
    );
  }

  function MoreButton() {
    return activeSearchText !== undefined ? (
      <TextButton
        text={"Show more"}
        width="100%"
        onClick={handleApi(async () => {
          const findResponse = await api.findCards(
            activeSearchText,
            cards.length,
          );
          const filteredResponse = findResponse.cards.filter(
            (fc) => !cards.some((c) => c.id === fc.id),
          );
          setCards([...cards, ...filteredResponse]);
          setCount(findResponse.count);
        })}
      />
    ) : undefined;
  }

  function ViewModal(groomCard: GroomCard) {
    const { id } = groomCard;
    return (
      <div className="w3-modal" style={{ display: "block" }}>
        <div className="w3-modal-content">
          <GroomItemView
            {...groomCard}
            onBack={() => setModal({ kind: "none" })}
            onDelete={handleApi(async () => {
              await api.deleteCard(id);
              const newCards = cards.filter((c) => c.id !== id);
              if (newCards.length !== cards.length) setCards(newCards);
              setModal({ kind: "none" });
            })}
            onDeleteHistory={handleApi(async () => {
              await api.deleteHistory(id);
              setModal({
                kind: "view",
                card: { ...groomCard, state: "new" },
              });
            })}
            onDisable={handleApi(async () => {
              await api.disable(id);
              const newCards = cards.map((c) =>
                c.id !== id ? c : { ...c, disabled: true },
              );
              setCards(newCards);
              setModal({ kind: "none" });
            })}
            onEnable={handleApi(async () => {
              await api.enable(id);
              const newCards = cards.map((c) =>
                c.id !== id ? c : { ...c, disabled: false },
              );
              setCards(newCards);
              setModal({ kind: "none" });
            })}
            onEdit={() => setModal({ kind: "edit", card: groomCard })}
          ></GroomItemView>
        </div>
      </div>
    );
  }

  function EditModal(groomCard: GroomCard) {
    const { id, prompt, solution } = groomCard;
    return (
      <div className="w3-modal" style={{ display: "block" }}>
        <div className="w3-modal-content">
          <EditView
            id={id}
            prompt={prompt}
            solution={solution}
            onCancel={handleApi(async () => {
              setModal({ kind: "view", card: groomCard });
              await api.deleteAutoSave();
            })}
            onSave={handleApi(async (card) => {
              await api.updateCard(card);
              const newCards = cards.map((c) =>
                c.id !== id ? c : { ...c, prompt: card.prompt },
              );
              setCards(newCards);
              setModal({ kind: "view", card: { ...groomCard, ...card } });
            })}
            writeAutoSave={handleApi(api.writeAutoSave)}
          ></EditView>
        </div>
      </div>
    );
  }
}
