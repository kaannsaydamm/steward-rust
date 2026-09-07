// Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
// app/routes.ts sessions route (MIT). Modified for Steward: full-page
// session history over ListChatSessions/GetChatSession with type-to-filter.

"use client";

import { useMemo, useState } from "react";
import { stewardClient, timeAgo } from "@/lib/types";
import type { ChatSessionSummary } from "@/lib/proto/steward";
import { useTranslation } from "@/lib/i18n/context";

export default function SessionsTab({ onOpenChat }: { readonly onOpenChat?: () => void }) {
  const { t } = useTranslation();
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([]);
  const [query, setQuery] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");

  async function load() {
    try {
      const response = await stewardClient.listChatSessions({ limit: 100 });
      setSessions(response.sessions);
      setLoaded(true);
      setError("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  async function open(id: string) {
    await stewardClient.getChatSession({ sessionId: id, sinceSequence: 0 });
    onOpenChat?.();
  }

  async function remove(id: string) {
    await stewardClient.deleteChatSession({ sessionId: id });
    await load();
  }

  if (!loaded && !error) {
    void load();
  }

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return sessions;
    return sessions.filter(
      (session) =>
        session.title.toLowerCase().includes(needle) ||
        session.model.toLowerCase().includes(needle) ||
        session.sessionId.toLowerCase().includes(needle),
    );
  }, [sessions, query]);

  return (
    <section className="mx-auto w-full max-w-5xl px-4 py-8 md:px-6">
      <header className="mb-6">
        <h2 className="font-serif text-2xl text-on-surface">{t("sessions.title")}</h2>
        <p className="mt-1 text-sm text-on-surface-variant/60">{t("sessions.subtitle")}</p>
      </header>
      <input
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder={t("sessions.searchPlaceholder")}
        className="mb-4 w-full border border-outline-variant/40 bg-surface-container-low px-4 py-2.5 text-sm text-on-surface outline-none focus:border-primary/60"
      />
      {error && (
        <p role="alert" className="border border-error/40 p-3 text-sm text-error">
          {error}
        </p>
      )}
      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-on-surface-variant/50">{t("sessions.empty")}</p>
      ) : (
        <ul className="divide-y divide-outline-variant/15 border border-outline-variant/20">
          {filtered.map((session) => (
            <li key={session.sessionId} className="group flex items-center gap-3 px-4 py-3 hover:bg-surface-container/60">
              <button type="button" onClick={() => void open(session.sessionId)} className="min-w-0 flex-1 text-left">
                <span className="block truncate text-sm text-on-surface">{session.title}</span>
                <span className="block truncate font-mono text-[10px] text-outline">
                  {session.model} · {timeAgo(session.updatedAt)} · {session.sessionId.slice(0, 12)}
                </span>
              </button>
              <button
                type="button"
                aria-label={t("chat.deleteSession")}
                title={t("chat.deleteSession")}
                onClick={() => void remove(session.sessionId)}
                className="shrink-0 px-2 py-2 text-outline opacity-0 group-hover:opacity-100 hover:text-error"
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
