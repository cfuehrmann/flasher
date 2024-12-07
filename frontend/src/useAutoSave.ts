import { useEffect, useState, useRef } from "react";
import { Card } from "./types";

export function useAutoSave(
  initialCard: Card,
  writeAutoSave: (c: Card) => Promise<void>,
) {
  const [card, setCard] = useState(initialCard);

  const isSaving = useRef(false);
  const cardRef = useRef(card);
  const interval = useRef<number>(undefined);

  useEffect(() => {
    cardRef.current = card;
  });

  useEffect(() => {
    startAutoSaveInterval();
    return clearAutoSaveInterval;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    card,
    setPrompt,
    setSolution,
    startAutoSaveInterval,
    clearAutoSaveInterval,
  };

  function startAutoSaveInterval() {
    interval.current = setInterval(async () => {
      if (isSaving.current) return;
      isSaving.current = true;
      await writeAutoSave(cardRef.current);
      isSaving.current = false;
    }, 5000);
  }

  function clearAutoSaveInterval() {
    if (interval.current) clearInterval(interval.current);
  }

  // Narratives for this component. Not pulled out because too trivial
  function setPrompt(prompt: string) {
    setCard({ ...card, prompt });
  }
  function setSolution(solution: string) {
    setCard({ ...card, solution });
  }
}
