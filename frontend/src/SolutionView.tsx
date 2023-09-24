import ReactMarkdown from "react-markdown";
import remarkMath from "remark-math";
import remarkGfm from "remark-gfm";
import rehypeKatex from "rehype-katex";

export function SolutionView(props: { solution: string }) {
  return (
    <>
      <div className="w3-container markdown-body">
        <ReactMarkdown
          remarkPlugins={[remarkMath, remarkGfm]}
          rehypePlugins={[rehypeKatex]}
        >
          {props.solution}
        </ReactMarkdown>
      </div>
    </>
  );
}
