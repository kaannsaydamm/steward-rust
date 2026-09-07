// Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
// session-picker.tsx overlay semantics (MIT). Modified for Steward: opens
// with Ctrl+X in ChatTab, filters over ListChatSessions, Enter opens.

"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { stewardClient, timeAgo } from "@/lib/types";
import type { ChatSessionSummary } from "@/lib/proto/steward";
import { useTranslation } from "@/lib/i18n/context";

export default function SessionSwitcher({
  open,
  onClose,
  onOpenSession,
}: {
  readonly open: boolean;
  readonly onClose: () => void;
  readonly onOpenSession: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([]);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    stewardClient
      .listChatSessions({ limit: 50 })
      .then((response) => setSessions(response.sessions))
      .catch(() => setSessions([]));
    setQuery("");
    setActiveIndex(0);
    window.setTimeout(() => inputRef.current?.focus(), 0);
  }, [open]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return sessions;
    return sessions.filter(
      (session) =>
        session.title.toLowerCase().includes(needle) ||
        session.model.toLowerCase().includes(needle),
    );
  }, [sessions, query]);

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(filtered.length - 1, 0)));
  }, [filtered.length]);

  if (!open) return null;

  function commit(session: ChatSessionSummary | undefined) {
    if (!session) return;
    onOpenSession(session.sessionId);
    onClose();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("chat.switcher.title")}
        className="composer-surface w-full max-w-xl shadow-[0_20px_60px_rgba(0,0,0,0.6)]"
        onClick={(event) => event.stopPropagation()}
      >
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              onClose();
            } else if (event.key === "ArrowDown") {
              event.preventDefault();
              if (filtered.length > 0) setActiveIndex((index) => (index + 1) % filtered.length);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              if (filtered.length > 0) setActiveIndex((index) => (index - 1 + filtered.length) % filtered.length);
            } else if (event.key === "Enter") {
              event.preventDefault();
              commit(filtered[activeIndex]);
            }
          }}
          placeholder={t("chat.switcher.placeholder")}
          className="w-full border-b border-outline-variant/20 bg-transparent px-4 py-3 text-sm text-on-surface outline-none"
        />
        <div className="max-h-80 overflow-y-auto">
          {filtered.length === 0 ? (
            <p className="px-4 py-3 font-mono text-xs text-on-surface-variant/50">{t("sessions.empty")}</p>
          ) : (
            filtered.map((session, index) => {
              const active = index === activeIndex;
              return (
                <button
                  key={session.sessionId}
                  type="button"
                  role="option"
                  aria-selected={active}
                  data-active={active}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => commit(session)}
                  onMouseEnter={() => setActiveIndex(index)}
                  className={`flex w-full cursor-pointer items-center gap-3 px-4 py-2.5 text-left transition-colors duration-150 ${active ? "bg-primary/10" : ""}`}
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-on-surface">{session.title}</span>
                    <span className="block truncate font-mono text-[10px] text-outline">
                      {session.model} · {timeAgo(session.updatedAt)}
                    </span>
                  </span>
                  {active && <span className="shrink-0 font-mono text-[10px] text-outline">&crarr;</span>}
                </button>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
