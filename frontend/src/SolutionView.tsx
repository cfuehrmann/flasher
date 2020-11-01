import * as React from "react";
// @ts-ignore
import math from "micromark-extension-math";
// @ts-ignore
import mathHtml from "micromark-extension-math/html";
// @ts-ignore
import gfmSyntax from "micromark-extension-gfm";
// @ts-ignore
import gfmHtml from "micromark-extension-gfm/html";
import micromark from "micromark";

export function SolutionView(props: { solution: string }) {
  const markdown = micromark(props.solution, {
    extensions: [math, gfmSyntax()],
    htmlExtensions: [mathHtml(), gfmHtml],
  });

  return (
    <>
      <div
        className="w3-container markdown-body"
        dangerouslySetInnerHTML={{ __html: markdown }}
      />
    </>
  );
}
