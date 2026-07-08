"use client";

import { useState } from "react";
import { ChevronDown, ChevronRight, Wrench } from "lucide-react";
import { useTranslation } from "@/lib/i18n/context";

interface ToolCall {
  toolName: string;
  content: string;
}

/** Hermes/Claude Code-style compact strip for tool activity: a one-line header with per-call
 * pills, expandable to reveal each call's raw output. Keeps the transcript readable instead of
 * interleaving full tool dumps between prose. */
export default function ToolStepGroup({ calls }: { readonly calls: ToolCall[] }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [expandedCall, setExpandedCall] = useState(-1);

  const uniqueNames = [...new Set(calls.map((call) => call.toolName).filter(Boolean))];

  return (
    <div className="border border-outline-variant/30 bg-surface-container-low/50">
      <button
        type="button"
        onClick={() => setOpen((current) => !current)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-surface-container"
      >
        {open ? (
          <ChevronDown className="h-3 w-3 shrink-0 text-outline" strokeWidth={2} />
        ) : (
          <ChevronRight className="h-3 w-3 shrink-0 text-outline" strokeWidth={2} />
        )}
        <Wrench className="h-3 w-3 shrink-0 text-primary/70" strokeWidth={1.75} />
        <span className="font-mono text-[10px] uppercase tracking-widest text-outline">
          {t("chat.toolSteps", { count: String(calls.length) })}
        </span>
        <span className="flex min-w-0 flex-wrap gap-1.5">
          {uniqueNames.map((name) => (
            <span
              key={name}
              className="truncate border border-primary/25 bg-primary/5 px-2 py-0.5 font-mono text-[10px] text-primary/90"
            >
              {name}
            </span>
          ))}
        </span>
      </button>
      {open && (
        <div className="border-t border-outline-variant/20">
          {calls.map((call, index) => (
            <div key={index} className="border-b border-outline-variant/10 last:border-b-0">
              <button
                type="button"
                onClick={() => setExpandedCall((current) => (current === index ? -1 : index))}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left hover:bg-surface-container"
              >
                <span className="font-mono text-[10px] text-primary/80">{call.toolName || "tool"}</span>
                <span className="min-w-0 flex-1 truncate text-[11px] text-on-surface-variant/50">
                  {call.content.split("\n")[0]}
                </span>
              </button>
              {expandedCall === index && (
                <pre className="max-h-64 overflow-auto border-t border-outline-variant/10 bg-surface-container-lowest px-3 py-2 font-mono text-[11px] leading-5 text-on-surface-variant/80 whitespace-pre-wrap">
                  {call.content}
                </pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
