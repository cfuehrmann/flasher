import { useEffect, useState, useRef } from "react";
import { Card } from "./types";

export function useAutoSave(
  initialCard: Card,
  writeAutoSave: (c: Card) => Promise<void>,
) {
  const [card, setCard] = useState(initialCard);

  const isSaving = useRef(false);
  const cardRef = useRef(card);

  useEffect(() => {
    cardRef.current = card;
  });

  useEffect(() => {
    const interval = setInterval(async () => {
      if (isSaving.current) return;

      isSaving.current = true;
      await writeAutoSave(cardRef.current);
      isSaving.current = false;
    }, 5000);
    return () => {
      clearInterval(interval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { card, setPrompt, setSolution };

  // Narratives for this component. Not pulled out because too trivial
  function setPrompt(prompt: string) {
    setCard({ ...card, prompt });
  }
  function setSolution(solution: string) {
    setCard({ ...card, solution });
  }
}
