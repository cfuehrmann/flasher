import * as React from "react";
import MarkDownIt from "markdown-it";
// @ts-ignore
import KaTeX from "markdown-it-katex";

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
