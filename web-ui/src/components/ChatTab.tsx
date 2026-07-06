"use client";

import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { stewardClient } from "@/lib/grpc";
import { timeAgo } from "@/lib/types";
import { ChatEventKind, type ChatMessageInfo, type ChatSessionSummary } from "@/lib/proto/steward";

type DisplayMessage = Pick<ChatMessageInfo, "role" | "content" | "toolName">;

export default function ChatTab({ onNavigateToProviders }: { readonly onNavigateToProviders?: () => void }) {
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([]);
  const [sessionId, setSessionId] = useState("");
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [activeModel, setActiveModel] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      stewardClient
        .listProviderProfiles({})
        .then((response) => {
          const active = response.profiles.find((profile) => profile.active);
          if (active) setActiveModel(active.model);
        })
        .catch(() => undefined);
    }, 0);
    return () => window.clearTimeout(timer);
  }, []);

  const toolCalls = messages.filter((message) => message.role === "tool");

  const loadSessions = useCallback(async () => {
    const response = await stewardClient.listChatSessions({ limit: 30 });
    setSessions(response.sessions);
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadSessions().catch((reason) => setError(String(reason)));
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadSessions]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  async function openSession(id: string) {
    const session = await stewardClient.getChatSession({ sessionId: id });
    setSessionId(id);
    setMessages(session.messages);
    setError("");
  }

  async function removeSession(id: string) {
    await stewardClient.deleteChatSession({ sessionId: id });
    if (id === sessionId) {
      newSession();
    }
    await loadSessions();
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    const message = input.trim();
    if (!message || busy) return;
    setInput("");
    setBusy(true);
    setError("");
    setMessages((current) => [...current, { role: "user", content: message, toolName: "" }]);
    try {
      const stream = stewardClient.chat({
        sessionId,
        message,
        workingDirectory: "",
        allowTools: true,
      });
      for await (const chatEvent of stream) {
        if (chatEvent.kind === ChatEventKind.CHAT_EVENT_KIND_SESSION) {
          setSessionId(chatEvent.sessionId);
        } else if (chatEvent.kind === ChatEventKind.CHAT_EVENT_KIND_TEXT) {
          setMessages((current) => [
            ...current,
            { role: "assistant", content: chatEvent.content, toolName: "" },
          ]);
        } else if (
          chatEvent.kind === ChatEventKind.CHAT_EVENT_KIND_TOOL_START ||
          chatEvent.kind === ChatEventKind.CHAT_EVENT_KIND_TOOL_RESULT
        ) {
          setMessages((current) => [
            ...current,
            { role: "tool", content: chatEvent.content, toolName: chatEvent.toolName },
          ]);
        }
      }
      await loadSessions();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  function newSession() {
    setSessionId("");
    setMessages([]);
    setError("");
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <aside className="hidden w-72 shrink-0 overflow-y-auto border-r border-outline-variant/30 p-4 lg:block">
        <button className="btn-ghost mb-5 w-full" onClick={newSession}>+ New session</button>
        <p className="mb-3 font-label-mono text-[10px] uppercase tracking-widest text-outline">Recent sessions</p>
        <div className="space-y-1">
          {sessions.map((session) => (
            <div
              key={session.sessionId}
              className={`group flex items-center gap-1 border-l px-1 ${sessionId === session.sessionId ? "border-primary bg-primary/5" : "border-transparent hover:bg-surface-container"}`}
            >
              <button
                onClick={() => void openSession(session.sessionId)}
                className="min-w-0 flex-1 px-2 py-2 text-left"
              >
                <span className="block truncate text-sm text-on-surface">{session.title}</span>
                <span className="block truncate font-mono text-[10px] text-outline">
                  {session.model} · {timeAgo(session.updatedAt)}
                </span>
              </button>
              <button
                aria-label="Delete session"
                title="Delete session"
                onClick={() => void removeSession(session.sessionId)}
                className="shrink-0 px-2 py-2 text-outline opacity-0 group-hover:opacity-100 hover:text-error"
              >
                ×
              </button>
            </div>
          ))}
        </div>
      </aside>
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-outline-variant/20 px-4 py-3 md:px-6">
          <div>
            <h2 className="font-serif text-xl text-on-surface">Operator chat</h2>
            <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
              {sessionId ? `Session ${sessionId.slice(0, 12)}` : "New session"}
            </p>
          </div>
          <button className="btn-ghost lg:hidden" onClick={newSession}>New</button>
        </header>
        <div className="flex-1 overflow-y-auto px-4 py-6 md:px-[10%]">
          {messages.length === 0 && (
            <div className="mx-auto mt-[12vh] max-w-xl text-center">
              <pre className="mb-6 inline-block text-left font-mono text-primary">{"     _\n    ( )\n   [ - ]\n  /     \\\n |  ^w^  |\n [=======]\n   \\___\\"}</pre>
              <h3 className="font-serif text-2xl">What should Steward handle?</h3>
              <p className="mt-2 text-sm text-outline">Model responses, governed tool calls, and session history appear here.</p>
            </div>
          )}
          <div className="mx-auto max-w-4xl space-y-5">
            {messages.map((message, index) => (
              <article key={`${message.role}-${index}`} className={`border-l-2 pl-4 ${message.role === "user" ? "border-primary" : message.role === "tool" ? "border-outline" : "border-sigil-blue"}`}>
                <p className="mb-1 font-mono text-[10px] uppercase tracking-widest text-outline">
                  {message.role === "assistant" ? "Steward" : message.toolName || message.role}
                </p>
                <div className="whitespace-pre-wrap text-sm leading-6 text-on-surface">{message.content}</div>
              </article>
            ))}
            {busy && <p className="font-mono text-xs text-primary">Thinking...</p>}
            {error && (
              <div role="alert" className="flex items-center justify-between gap-3 border border-error/40 p-3 text-sm text-error">
                <span>{error}</span>
                {error.includes("no provider profile is active") && onNavigateToProviders && (
                  <button
                    type="button"
                    onClick={onNavigateToProviders}
                    className="shrink-0 border border-error/40 px-3 py-1 font-mono text-[10px] uppercase text-error"
                  >
                    Open Providers
                  </button>
                )}
              </div>
            )}
            <div ref={bottomRef} />
          </div>
        </div>
        <form onSubmit={submit} className="border-t border-outline-variant/20 p-3 md:px-[10%] md:py-5">
          <div className="input-glow mx-auto flex max-w-4xl items-end gap-3 border border-outline-variant/50 bg-surface-container-low px-4 py-3">
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
              rows={2}
              placeholder="Message Steward..."
              className="min-h-12 flex-1 resize-none bg-transparent text-sm text-on-surface outline-none"
            />
            <button disabled={busy || !input.trim()} className="btn-ghost disabled:cursor-not-allowed disabled:opacity-30">Send</button>
          </div>
        </form>
      </section>
      <aside className="hidden w-72 shrink-0 overflow-y-auto border-l border-outline-variant/30 p-4 xl:block">
        <div className="mb-5 border border-outline/20 p-4 card-ghost">
          <p className="mb-1 font-label-mono text-[10px] uppercase tracking-widest text-outline">Model</p>
          <p className="truncate font-mono text-sm text-on-surface">{activeModel || "—"}</p>
        </div>
        <div className="border border-outline/20 p-4 card-ghost">
          <p className="mb-3 font-label-mono text-[10px] uppercase tracking-widest text-outline">
            Tools / {toolCalls.length}
          </p>
          {toolCalls.length === 0 ? (
            <p className="text-center text-xs text-on-surface-variant/40">no tool calls yet</p>
          ) : (
            <div className="space-y-2">
              {toolCalls.map((call, index) => (
                <div key={index} className="border-b border-outline-variant/10 pb-2 last:border-0">
                  <p className="font-mono text-[11px] text-primary">{call.toolName}</p>
                  <p className="truncate text-[11px] text-on-surface-variant/50">{call.content}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
