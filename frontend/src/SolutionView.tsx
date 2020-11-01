import * as React from "react";
import ReactMarkdown from "react-markdown";
import RemarkMathPlugin from "remark-math";
import RemarkGfmPlugin from "remark-gfm";
// @ts-ignore
import { BlockMath, InlineMath } from "react-katex";

export function SolutionView(props: { solution: string }) {
  return (
    <>
      <div className="w3-container markdown-body">
        <ReactMarkdown
          //escapeHtml={false}
          plugins={[RemarkMathPlugin, RemarkGfmPlugin]}
          renderers={{
            inlineMath: ({ value }) => <InlineMath>{value}</InlineMath>,
            math: ({ value }) => <BlockMath>{value}</BlockMath>,
          }}
        >
          {props.solution}
        </ReactMarkdown>
      </div>
    </>
  );
}
