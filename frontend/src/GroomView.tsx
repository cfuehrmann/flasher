import * as React from "react";
import { useEffect } from "react";

import { TextButton } from "./Buttons";
import { FindResponse, FindResponseCard } from "./types";

export function GroomView(props: {
  onGoToPrompt: () => void;
  onChangeInput: (searchText: string) => void;
  onGroomItem: (id: string) => void;
  onGoToCreate: () => void;
  onGoToPage: (searchText: string, page: number) => void;
  searchText: string;
  page?: number;
  findResponse: FindResponse;
}) {
  useEffect(() => window.scrollTo(0, 0));

  const { cards, count, pageCount } = props.findResponse;
  const page = props.page;

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
          placeholder="Search..."
        />
      </div>
      <TextButton key="xxx" text="+" onClick={props.onGoToCreate} />
      <br />
      {count !== undefined
        ? `There are ${count} results on ${pageCount} pages.`
        : ""}
      <br />
      {List()}
      <br />
      {f()}
    </>
  );

  function List() {
    return cards.map(getCardButton);
  }

  function getCardButton(c: FindResponseCard) {
    const text = (c.disabled ? "!!! " : "") + c.prompt;
    return (
      <TextButton
        key={c.id}
        width="100%"
        text={text}
        onClick={() => props.onGroomItem(c.id)}
      />
    );
  }

  function f() {
    if (count === undefined || pageCount === undefined || page === undefined)
      return "";

    let start = 0;
    let end = pageCount;

    if (page - 3 < 0) {
      if (page + 3 >= pageCount) {
        start = 0;
        end = pageCount;
      } else {
        start = 0;
        end = 7;
      }
    } else {
      if (page + 3 >= pageCount) {
        start = pageCount - 7;
        end = pageCount;
      } else {
        start = page - 3;
        end = page + 4;
      }
    }

    return (
      <>
        {/* {page > 0 ? (
          <TextButton
            key={"xxx1"}
            text={"<"}
            onClick={() => props.onGoToPage(props.searchText, page - 1)}
          ></TextButton>
        ) : (
          ""
        )} */}
        {range(start, page).map((p) => (
          <TextButton
            key={(p + 1).toString()}
            text={(p + 1).toString()}
            onClick={() => props.onGoToPage(props.searchText, p)}
          ></TextButton>
        ))}
        <TextButton
          key={(page + 1).toString()}
          text={(page + 1).toString()}
          onClick={() => undefined}
          disabled={true}
        ></TextButton>
        {range(page + 1, end).map((p) => (
          <TextButton
            key={(p + 1).toString()}
            text={(p + 1).toString()}
            onClick={() => props.onGoToPage(props.searchText, p)}
          ></TextButton>
        ))}
        {/* {page < pageCount - 1 ? (
          <TextButton
            key={"xxx2"}
            text={">"}
            onClick={() => props.onGoToPage(props.searchText, page + 1)}
          ></TextButton>
        ) : (
          ""
        )} */}
      </>
    );
  }
}

function range(start: number, end: number) {
  return Array.from({ length: end - start }).map((_, i) => start + i);
}
