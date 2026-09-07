"use client";

import { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight, Loader2, Wrench } from "lucide-react";
import { useTranslation } from "@/lib/i18n/context";

interface ToolCall {
  toolName: string;
  content: string;
  /** Client-side timestamp captured at TOOL_START. */
  at?: number;
  /** True while the call has no matching result yet. */
  pending?: boolean;
}

/** Format a latency span; returns null when no start time is known. */
function latencyLabel(startedAt: number | undefined, pending: boolean | undefined): string | null {
  if (startedAt === undefined) return null;
  const milliseconds = pending ? Date.now() - startedAt : undefined;
  if (pending) return `${Math.max(1, Math.round((milliseconds ?? 0) / 100) / 10)}s`;
  return null;
}

/** Hermes/Claude Code-style compact strip for tool activity: a one-line header with per-call
 * pills, expandable to reveal each call's raw output. Keeps the transcript readable instead of
 * interleaving full tool dumps between prose. Pending calls render a spinner; each call shows
 * its latency once complete; expansion state is remembered per call index while mounted. */
export default function ToolStepGroup({ calls }: { readonly calls: ToolCall[] }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  // Expansion memory per call id so re-renders keep user choices.
  const [expandedCalls, setExpandedCalls] = useState<Set<number>>(new Set());
  // Ticks once per 250ms while any call is pending, driving latency labels.
  const [, setTick] = useState(0);
  const hasPending = calls.some((call) => call.pending);
  const timerRef = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  useEffect(() => {
    if (!hasPending) return;
    timerRef.current = setInterval(() => setTick((value) => value + 1), 250);
    return () => clearInterval(timerRef.current);
  }, [hasPending]);

  function toggleExpanded(index: number) {
    setExpandedCalls((current) => {
      const next = new Set(current);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }

  const uniqueNames = [...new Set(calls.map((call) => call.toolName).filter(Boolean))];
  const completedCount = calls.filter((call) => !call.pending).length;

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
        {hasPending && (
          <Loader2 className="h-3 w-3 shrink-0 animate-spin text-primary" strokeWidth={2} />
        )}
        {completedCount === calls.length && calls.length > 0 && (
          <span className="shrink-0 font-mono text-[10px] text-outline">
            {t("chat.toolGroup.total", { count: String(calls.length) })}
          </span>
        )}
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
          {calls.map((call, index) => {
            const latency = call.pending ? null : latencyLabel(call.at, false);
            return (
              <div key={index} className="border-b border-outline-variant/10 last:border-b-0">
                <button
                  type="button"
                  onClick={() => toggleExpanded(index)}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left hover:bg-surface-container"
                >
                  {call.pending ? (
                    <Loader2 className="h-3 w-3 shrink-0 animate-spin text-primary/80" strokeWidth={2} />
                  ) : (
                    <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary/60" />
                  )}
                  <span className="font-mono text-[10px] text-primary/80">{call.toolName || "tool"}</span>
                  <span className="min-w-0 flex-1 truncate text-[11px] text-on-surface-variant/50">
                    {call.content.split("\n")[0]}
                  </span>
                  {latency && (
                    <span className="shrink-0 font-mono text-[10px] text-outline">{latency}</span>
                  )}
                </button>
                {expandedCalls.has(index) && (
                  <pre className="max-h-64 overflow-auto border-t border-outline-variant/10 bg-surface-container-lowest px-3 py-2 font-mono text-[11px] leading-5 text-on-surface-variant/80 whitespace-pre-wrap">
                    {call.content}
                  </pre>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
