"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import "highlight.js/styles/github-dark-dimmed.css";

/** Rich rendering for model responses — headings, lists, tables, fenced code with syntax
 * highlighting — replacing the old whitespace-pre-wrap plain text. */
export default function Markdown({ content }: { readonly content: string }) {
  return (
    <div className="markdown-body text-sm leading-6 text-on-surface">
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
