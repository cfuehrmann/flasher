import ReactMarkdown from "react-markdown";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex"; // // @ts-ignore

export function SolutionView(props: { solution: string }) {
  return (
    <>
      <div className="w3-container markdown-body">
        <ReactMarkdown
          remarkPlugins={[remarkMath]}
          rehypePlugins={[rehypeKatex]}
        >
          {props.solution}
        </ReactMarkdown>
      </div>
    </>
  );
}
