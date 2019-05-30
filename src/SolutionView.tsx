import MarkDownIt = require("markdown-it");
import KaTeX = require("markdown-it-katex");
import * as React from "preact";

export function SolutionView(props: { solution: string }) {
  const md = new MarkDownIt();
  md.use(KaTeX);
  return (
    <>
      <div
        className="w3-container markdown-body"
        dangerouslySetInnerHTML={{ __html: md.render(props.solution) }}
      />
    </>
  );
}
