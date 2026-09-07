"use client";

import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { Check, ChevronDown, FoldVertical, MoveHorizontal, Plus, Type } from "lucide-react";
import { stewardClient } from "@/lib/grpc";
import { timeAgo } from "@/lib/types";
import {
  ChatEventKind,
  type ChatMessageInfo,
  type ChatSessionSummary,
  type PendingApproval,
  type ProviderProfileInfo,
} from "@/lib/proto/steward";
import { useTranslation } from "@/lib/i18n/context";
import Markdown from "./Markdown";
import ToolStepGroup from "./ToolStepGroup";
import CommandPalette, { rankCommands, type ChatCommand } from "./CommandPalette";
import SessionSwitcher from "./SessionSwitcher";
import type { TabId } from "./Sidebar";

type DisplayMessage = Pick<ChatMessageInfo, "role" | "content" | "toolName">;

type TranscriptBlock =
  | { kind: "message"; message: DisplayMessage }
  | { kind: "tools"; calls: DisplayMessage[] };

/** Collapse consecutive tool-role messages into one grouped block, matching how Hermes and
 * Claude Code compress tool activity into a short labeled strip instead of full transcript
 * entries. */
function groupTranscript(messages: DisplayMessage[]): TranscriptBlock[] {
  const blocks: TranscriptBlock[] = [];
  for (const message of messages) {
    const last = blocks[blocks.length - 1];
    if (message.role === "tool") {
      if (last?.kind === "tools") {
        last.calls.push(message);
      } else {
        blocks.push({ kind: "tools", calls: [message] });
      }
    } else {
      blocks.push({ kind: "message", message });
    }
  }
  return blocks;
}

const WIDTH_CLASSES = { narrow: "max-w-3xl", medium: "max-w-4xl", wide: "max-w-6xl" } as const;
const FONT_CLASSES = { small: "text-[13px]", medium: "", large: "text-[15px]" } as const;
type TranscriptWidth = keyof typeof WIDTH_CLASSES;
type TranscriptFont = keyof typeof FONT_CLASSES;

/** Auto-grow cap: ~6 rows of text-sm/leading-6 (24px per row). */
const TEXTAREA_MAX_HEIGHT_PX = 150;

function nextOf<T extends string>(options: readonly T[], current: T): T {
  return options[(options.indexOf(current) + 1) % options.length];
}

export default function ChatTab({
  onNavigate,
  daemonOnline = false,
}: {
  readonly onNavigate?: (tab: TabId) => void;
  readonly daemonOnline?: boolean;
}) {
  const { t } = useTranslation();
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([]);
  const [sessionId, setSessionId] = useState("");
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // Message queue (Hermes composer-queue semantics): Enter while busy parks
  // the message; the head auto-submits when the current turn's stream closes.
  const [queued, setQueued] = useState<string[]>([]);
  const [historyCursor, setHistoryCursor] = useState(-1);
  const [draftSnapshot, setDraftSnapshot] = useState("");
  const [activeModel, setActiveModel] = useState("");
  const [profiles, setProfiles] = useState<ProviderProfileInfo[]>([]);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [paletteIndex, setPaletteIndex] = useState(0);
  const [paletteDismissed, setPaletteDismissed] = useState(false);
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>([]);
  const [switchingProfile, setSwitchingProfile] = useState<string | null>(null);
  const [width, setWidth] = useState<TranscriptWidth>("medium");
  const [font, setFont] = useState<TranscriptFont>("medium");
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      const storedWidth = window.localStorage.getItem("steward.transcriptWidth");
      if (storedWidth && storedWidth in WIDTH_CLASSES) setWidth(storedWidth as TranscriptWidth);
      const storedFont = window.localStorage.getItem("steward.transcriptFont");
      if (storedFont && storedFont in FONT_CLASSES) setFont(storedFont as TranscriptFont);
    }, 0);
    return () => window.clearTimeout(timer);
  }, []);

  // Session switcher overlay: Ctrl+X toggles (Hermes session-picker trigger).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey && (event.key === "x" || event.key === "X")) {
        event.preventDefault();
        setSwitcherOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  function cycleWidth() {
    const next = nextOf(Object.keys(WIDTH_CLASSES) as TranscriptWidth[], width);
    setWidth(next);
    window.localStorage.setItem("steward.transcriptWidth", next);
  }

  function cycleFont() {
    const next = nextOf(Object.keys(FONT_CLASSES) as TranscriptFont[], font);
    setFont(next);
    window.localStorage.setItem("steward.transcriptFont", next);
  }

  const reloadProfiles = useCallback(async () => {
    const response = await stewardClient.listProviderProfiles({});
    setProfiles(response.profiles);
    const active = response.profiles.find((profile) => profile.active);
    if (active) setActiveModel(active.model);
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void reloadProfiles().catch(() => undefined);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [reloadProfiles]);

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

  // Approvals surface: poll the HITL registry while a turn runs (Hermes
  // approval.respond semantics — card appears for the tool awaiting consent).
  useEffect(() => {
    if (!busy) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const response = await stewardClient.listPendingApprovals({});
        if (!cancelled) setPendingApprovals(response.approvals);
      } catch {
        if (!cancelled) setPendingApprovals([]);
      }
    };
    void poll();
    const interval = window.setInterval(poll, 1_500);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [busy]);

  async function respondToApproval(approvalId: string, always: boolean, allow: boolean) {
    try {
      await stewardClient.respondApproval({
        approvalId,
        decision: allow ? (always ? 2 : 1) : 3,
      });
      setPendingApprovals((current) => current.filter((approval) => approval.approvalId !== approvalId));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }


  async function openSession(id: string) {
    const session = await stewardClient.getChatSession({ sessionId: id, sinceSequence: 0 });
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

  async function compactSession() {
    if (!sessionId || busy) return;
    setBusy(true);
    setError("");
    try {
      const response = await stewardClient.compactChatSession({ sessionId });
      const session = await stewardClient.getChatSession({ sessionId, sinceSequence: 0 });
      setMessages(session.messages);
      setMessages((current) => [
        ...current,
        {
          role: "assistant",
          content: t("chat.compacted", { count: String(response.removedMessages) }),
          toolName: "",
        },
      ]);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function activateProfile(profileId: string) {
    if (switchingProfile) return;
    setSwitchingProfile(profileId);
    setError("");
    try {
      const active = await stewardClient.activateProviderProfile({ profileId });
      setActiveModel(active.model);
      await reloadProfiles();
      setModelPickerOpen(false);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSwitchingProfile(null);
    }
  }

  async function sendMessage(message: string) {
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
      // Auto-drain: when the turn's stream closes, submit the queued head.
      setQueued((current) => {
        if (current.length === 0) return current;
        const [head, ...rest] = current;
        setQueued(rest);
        if (head) void sendMessage(head);
        return rest;
      });
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    const message = input.trim();
    if (!message || message.startsWith("/")) return;
    setInput("");
    setHistoryCursor(-1);
    setDraftSnapshot("");
    if (busy) {
      // Queue instead of dropping: Hermes composer-queue semantics.
      setQueued((current) => [...current, message]);
      return;
    }
    await sendMessage(message);
  }

  function recallHistory(direction: -1 | 1) {
    // Derive the user-text ring newest-first (composer-input-history port).
    const ring = messages
      .filter((message) => message.role === "user")
      .map((message) => message.content);
    if (ring.length === 0) return;
    if (direction === -1) {
      if (historyCursor === -1) {
        setDraftSnapshot(input);
        setHistoryCursor(0);
        setInput(ring[0] ?? "");
      } else if (historyCursor < ring.length - 1) {
        const next = historyCursor + 1;
        setHistoryCursor(next);
        setInput(ring[next] ?? "");
      }
    } else if (historyCursor > 0) {
      const next = historyCursor - 1;
      setHistoryCursor(next);
      setInput(ring[next] ?? "");
    } else if (historyCursor === 0) {
      setHistoryCursor(-1);
      setInput(draftSnapshot);
    }
  }

  function recallQueued(index: number) {
    setQueued((current) => current.filter((_, position) => position !== index));
    setHistoryCursor(-1);
    setInput(queued[index] ?? "");
    textareaRef.current?.focus();
  }

  function newSession() {
    setSessionId("");
    setMessages([]);
    setError("");
  }

  function runCommand(command: ChatCommand) {
    if (command.name === "help") {
      // Keep the palette open, listing every command with the hint line.
      setInput("/help");
      setPaletteIndex(0);
      setPaletteDismissed(false);
      textareaRef.current?.focus();
      return;
    }
    setInput("");
    setPaletteIndex(0);
    setPaletteDismissed(false);
    textareaRef.current?.focus();
    command.onRun();
  }

  // "tools" is the user-facing name for the capabilities tab.
  const chatCommands: ChatCommand[] = [
    { name: "new", description: t("chat.cmd.new"), onRun: () => newSession() },
    // Availability rule: compaction only makes sense with an active session.
    ...(sessionId ? [{ name: "compact", description: t("chat.cmd.compact"), onRun: () => void compactSession() } satisfies ChatCommand] : []),
    { name: "model", description: t("chat.cmd.model"), onRun: () => setModelPickerOpen(true) },
    { name: "help", description: t("chat.cmd.help"), onRun: () => undefined },
    { name: "dashboard", description: t("chat.cmd.dashboard"), onRun: () => onNavigate?.("dashboard") },
    { name: "providers", description: t("chat.cmd.providers"), onRun: () => onNavigate?.("providers") },
    { name: "tools", description: t("chat.cmd.tools"), onRun: () => onNavigate?.("capabilities") },
    { name: "agents", description: t("chat.cmd.agents"), onRun: () => onNavigate?.("agents") },
    { name: "workflows", description: t("chat.cmd.workflows"), onRun: () => onNavigate?.("workflows") },
    { name: "cron", description: t("chat.cmd.cron"), onRun: () => onNavigate?.("cron") },
    { name: "artifacts", description: t("chat.cmd.artifacts"), onRun: () => onNavigate?.("artifacts") },
    { name: "knowledge", description: t("chat.cmd.knowledge"), onRun: () => onNavigate?.("knowledge") },
  ];

  const paletteOpen = input.startsWith("/") && !paletteDismissed;
  const { matches: paletteMatches, showAll: paletteShowAll } = rankCommands(chatCommands, input);
  const activeCommandIndex = Math.min(paletteIndex, Math.max(paletteMatches.length - 1, 0));

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <aside className="hidden w-72 shrink-0 overflow-y-auto border-r border-outline-variant/30 p-4 lg:block">
        <button className="btn-ghost mb-5 w-full" onClick={newSession}>{t("chat.newSession")}</button>
        <p className="mb-3 font-label-mono text-[10px] uppercase tracking-widest text-outline">{t("chat.recentSessions")}</p>
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
                aria-label={t("chat.deleteSession")}
                title={t("chat.deleteSession")}
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
            <h2 className="font-serif text-xl text-on-surface">{t("chat.operatorChat")}</h2>
            <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
              {sessionId ? t("chat.sessionLabel", { id: sessionId.slice(0, 12) }) : t("chat.newSessionShort")}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="btn-ghost gap-1.5"
              onClick={cycleFont}
              title={t("chat.fontTitle")}
              aria-label={t("chat.fontTitle")}
            >
              <Type className="h-3.5 w-3.5" strokeWidth={1.75} />
              <span>{font[0].toUpperCase()}</span>
            </button>
            <button
              type="button"
              className="btn-ghost gap-1.5"
              onClick={cycleWidth}
              title={t("chat.widthTitle")}
              aria-label={t("chat.widthTitle")}
            >
              <MoveHorizontal className="h-3.5 w-3.5" strokeWidth={1.75} />
              <span>{width[0].toUpperCase()}</span>
            </button>
            {sessionId && (
              <button
                type="button"
                className="btn-ghost gap-1.5"
                disabled={busy}
                onClick={() => void compactSession()}
                title={t("chat.compactTitle")}
                aria-label={t("chat.compactTitle")}
              >
                <FoldVertical className="h-3.5 w-3.5" strokeWidth={1.75} />
                <span>{t("chat.compact")}</span>
              </button>
            )}
            <button
              type="button"
              className="btn-ghost lg:hidden"
              onClick={newSession}
              title={t("chat.newSessionShort")}
              aria-label={t("chat.newSessionShort")}
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
            </button>
          </div>
        </header>
        <div className="flex-1 overflow-y-auto px-4 py-6 md:px-[10%]">
          {messages.length === 0 && (
            <div className="mx-auto mt-[10vh] max-w-xl text-center">
              <pre className="mascot-float mb-6 inline-block text-left font-mono text-sm leading-5 text-primary/90">{"     _\n    ( )\n   [ - ]\n  /     \\\n | ^w^  |\n [=======]\n   \\___\""}</pre>
              <h3 className="font-serif text-2xl text-on-surface">{t("chat.emptyTitle")}</h3>
              <p className="mt-2 text-sm leading-6 text-on-surface-variant/60">{t("chat.emptyBody")}</p>
              <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
                {["Summarize this repo", "Plan a refactor", "Run the test suite"].map((hint) => (
                  <button
                    key={hint}
                    type="button"
                    onClick={() => setInput(hint)}
                    className="row-interactive border border-outline/25 px-3 py-1.5 font-mono text-[11px] text-on-surface-variant/70 hover:border-primary/40 hover:text-primary"
                  >
                    {hint}
                  </button>
                ))}
              </div>
            </div>
          )}
          <div className={`mx-auto space-y-5 ${WIDTH_CLASSES[width]} ${FONT_CLASSES[font]}`}>
            {groupTranscript(messages).map((block, index) =>
              block.kind === "tools" ? (
                <ToolStepGroup key={`tools-${index}`} calls={block.calls} />
              ) : (
                <article
                  key={`${block.message.role}-${index}`}
                  className={`msg-enter border-l-2 pl-4 ${block.message.role === "user" ? "border-primary bg-primary/[0.03]" : "border-sigil-blue"}`}
                >
                  <p className={`mb-1 font-mono text-[10px] uppercase tracking-widest ${block.message.role === "assistant" ? "text-primary/70" : "text-outline"}`}>
                    {block.message.role === "assistant" ? "Steward" : block.message.role}
                  </p>
                  {block.message.role === "assistant" ? (
                    <Markdown content={block.message.content} />
                  ) : (
                    <div className="whitespace-pre-wrap text-sm leading-6 text-on-surface">
                      {block.message.content}
                    </div>
                  )}
                </article>
              )
            )}
            {pendingApprovals.map((approval) => (
              <div
                key={approval.approvalId}
                role="alertdialog"
                aria-label={t("chat.approval.title")}
                className="border border-warning/50 bg-warning/[0.04] p-4"
              >
                <p className="mb-1 font-label-mono text-[10px] uppercase tracking-widest text-warning">
                  {t("chat.approval.title")}
                </p>
                <p className="mb-3 text-sm text-on-surface">
                  <span className="font-mono text-primary">{approval.toolName}</span>
                  {approval.summary ? ` — ${approval.summary}` : ""}
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    disabled={!daemonOnline}
                    onClick={() => void respondToApproval(approval.approvalId, false, true)}
                    className="btn-ghost border border-primary/40 px-3 py-1.5 font-mono text-[11px] uppercase text-primary disabled:opacity-40"
                  >
                    {t("chat.approval.allowOnce")}
                  </button>
                  <button
                    type="button"
                    disabled={!daemonOnline}
                    onClick={() => void respondToApproval(approval.approvalId, true, true)}
                    className="btn-ghost border border-outline-variant/40 px-3 py-1.5 font-mono text-[11px] uppercase text-on-surface-variant/80 disabled:opacity-40"
                  >
                    {t("chat.approval.allowAlways")}
                  </button>
                  <button
                    type="button"
                    disabled={!daemonOnline}
                    onClick={() => void respondToApproval(approval.approvalId, false, false)}
                    className="btn-ghost border border-error/40 px-3 py-1.5 font-mono text-[11px] uppercase text-error disabled:opacity-40"
                  >
                    {t("chat.approval.deny")}
                  </button>
                </div>
              </div>
            ))}

            {busy && (
              <div className="flex items-center gap-2 pl-1">
                <span className="thinking-dots flex items-center">
                  <span />
                  <span />
                  <span />
                </span>
                <span className="font-mono text-[11px] uppercase tracking-widest text-primary/80">
                  {t("chat.thinking")}
                </span>
              </div>
            )}
            {error && (
              <div role="alert" className="flex items-center justify-between gap-3 border border-error/40 p-3 text-sm text-error">
                <span>{error}</span>
                {error.includes("no provider profile is active") && onNavigate && (
                  <button
                    type="button"
                    onClick={() => onNavigate("providers")}
                    className="shrink-0 border border-error/40 px-3 py-1 font-mono text-[10px] uppercase text-error"
                  >
                    {t("chat.openProviders")}
                  </button>
                )}
              </div>
            )}
            <div ref={bottomRef} />
          </div>
        </div>
        {queued.length > 0 && (
          <div className="composer-panel mx-auto mb-2 max-w-4xl" role="list" aria-label={t("chat.queue.label")}>
            {queued.map((message, index) => (
              <div key={`${index}-${message.slice(0, 16)}`} role="listitem" className="group flex items-center gap-2 border-b border-outline-variant/10 px-4 py-2 last:border-0">
                <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-on-surface-variant/70">{message}</span>
                <button
                  type="button"
                  onClick={() => recallQueued(index)}
                  title={t("chat.queue.recall")}
                  aria-label={t("chat.queue.recall")}
                  className="shrink-0 px-1 font-mono text-[10px] text-outline opacity-0 group-hover:opacity-100 hover:text-primary"
                >
                  ↑
                </button>
              </div>
            ))}
          </div>
        )}
        <form onSubmit={submit} className="border-t border-outline-variant/20 p-3 md:px-[10%] md:py-5">
          <div className="composer-surface input-glow relative mx-auto flex max-w-4xl items-end gap-3 px-4 py-3">
            {paletteOpen && (
              <CommandPalette
                commands={paletteMatches}
                activeIndex={activeCommandIndex}
                showHint={paletteShowAll}
                onSelect={runCommand}
                onHover={setPaletteIndex}
              />
            )}
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(event) => {
                setInput(event.target.value);
                setHistoryCursor(-1);
                setPaletteIndex(0);
                setPaletteDismissed(false);
              }}
              onKeyDown={(event) => {
                if (paletteOpen) {
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    if (paletteMatches.length === 0) return;
                    const delta = event.key === "ArrowDown" ? 1 : -1;
                    setPaletteIndex((index) => (index + delta + paletteMatches.length) % paletteMatches.length);
                    return;
                  }
                  if ((event.key === "Enter" && !event.shiftKey) || event.key === "Tab") {
                    event.preventDefault();
                    const command = paletteMatches[activeCommandIndex];
                    if (command && command.name !== "help") runCommand(command);
                    return;
                  }
                  if (event.key === "Escape") {
                    event.preventDefault();
                    setPaletteIndex(0);
                    setPaletteDismissed(true);
                    return;
                  }
                }
                if (event.key === "ArrowUp" && !event.shiftKey && input === "") {
                  event.preventDefault();
                  recallHistory(-1);
                  return;
                }
                if (event.key === "ArrowDown" && !event.shiftKey && historyCursor !== -1) {
                  event.preventDefault();
                  recallHistory(1);
                  return;
                }
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  if (input.startsWith("/")) {
                    // Command intent with the palette dismissed: reopen instead of sending.
                    setPaletteDismissed(false);
                    return;
                  }
                  event.currentTarget.form?.requestSubmit();
                }
              }}
              rows={2}
              placeholder={t("chat.messagePlaceholder")}
              className="min-h-12 flex-1 resize-none overflow-hidden bg-transparent text-sm leading-6 text-on-surface outline-none"
            />
            <button
              disabled={busy && queued.length === 0 && !input.trim()}
              className="btn-ghost disabled:cursor-not-allowed disabled:opacity-30 disabled:hover:bg-transparent"
            >
              {busy ? (queued.length > 0 ? `+${queued.length}` : "···") : t("chat.send")}
            </button>
          </div>
        </form>
        <div className="composer-fill flex items-center justify-between gap-3 px-3 pb-2 font-mono text-[10px] uppercase tracking-widest text-outline md:px-[10%]">
          <span className="truncate">{sessionId ? sessionId.slice(0, 12) : t("chat.newSessionShort")}</span>
          <span className="flex shrink-0 items-center gap-2">
            <span className="text-on-surface-variant/70">{activeModel || "—"}</span>
            <span>·</span>
            <span>{t("chat.status.toolCalls", { count: String(toolCalls.length) })}</span>
            <span className={`inline-block h-1.5 w-1.5 rounded-full ${daemonOnline ? "bg-primary dot-live" : "bg-error"}`} />
          </span>
        </div>
      </section>
      <aside className="hidden w-72 shrink-0 overflow-y-auto border-l border-outline-variant/30 p-4 xl:block">
        <div className="relative mb-5 border border-outline/20 p-4 card-ghost">
          <button
            type="button"
            onClick={() => setModelPickerOpen((open) => !open)}
            aria-expanded={modelPickerOpen}
            aria-haspopup="listbox"
            className="flex w-full cursor-pointer items-center justify-between gap-2 text-left"
          >
            <span className="font-label-mono text-[10px] uppercase tracking-widest text-outline">{t("chat.model")}</span>
            <ChevronDown
              className={`h-3.5 w-3.5 shrink-0 text-outline transition-transform duration-150 ${modelPickerOpen ? "rotate-180" : ""}`}
              strokeWidth={1.75}
            />
          </button>
          <p className="mt-1 truncate font-mono text-sm text-on-surface">{activeModel || "—"}</p>
          {modelPickerOpen && (
            <>
              <div className="fixed inset-0 z-20" aria-hidden="true" onClick={() => setModelPickerOpen(false)} />
              <div
                role="listbox"
                aria-label={t("chat.model")}
                className="absolute inset-x-0 top-full z-30 mt-2 max-h-60 overflow-y-auto border border-outline/30 bg-surface-container-low shadow-[0_10px_30px_rgba(0,0,0,0.45)]"
              >
                {profiles.length === 0 ? (
                  <p className="px-4 py-3 font-mono text-xs text-on-surface-variant/50">{t("chat.modelPicker.empty")}</p>
                ) : (
                  profiles.map((profile) => (
                    <button
                      key={profile.profileId}
                      type="button"
                      role="option"
                      aria-selected={profile.active}
                      disabled={switchingProfile !== null}
                      onClick={() => void activateProfile(profile.profileId)}
                      className={`flex w-full cursor-pointer items-center justify-between gap-2 px-4 py-2 text-left transition-colors duration-150 hover:bg-primary/10 disabled:cursor-wait ${profile.active ? "bg-primary/5" : ""}`}
                    >
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-mono text-xs text-on-surface">{profile.model || profile.profileId}</span>
                        <span className="block truncate font-mono text-[10px] text-outline">{profile.providerId}</span>
                      </span>
                      {profile.active && <Check className="h-3.5 w-3.5 shrink-0 text-primary" strokeWidth={2} />}
                    </button>
                  ))
                )}
              </div>
            </>
          )}
        </div>
        <div className="border border-outline/20 p-4 card-ghost">
          <p className="mb-3 font-label-mono text-[10px] uppercase tracking-widest text-outline">
            {t("chat.tools")} / {toolCalls.length}
          </p>
          {toolCalls.length === 0 ? (
            <p className="text-center text-xs text-on-surface-variant/40">{t("chat.noToolCalls")}</p>
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
      <SessionSwitcher
        open={switcherOpen}
        onClose={() => setSwitcherOpen(false)}
        onOpenSession={(id) => void openSession(id)}
      />
    </div>
  );
}
