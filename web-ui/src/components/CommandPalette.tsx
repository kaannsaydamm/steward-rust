"use client";

import { useEffect, useRef } from "react";
import { useTranslation } from "@/lib/i18n/context";

export interface ChatCommand {
  name: string;
  description: string;
  onRun: () => void;
}

/** Description-aware tiered fuzzy scoring for the slash-command menu.
 *  Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
 *  ui-tui/src/app/slash/fuzzyScore.ts (MIT). Modified for Steward: query shape
 *  is `ChatCommand { name, description }` and ranking applies availability
 *  filtering upstream of scoring.
 */
export interface SlashScoreItem {
  readonly aliases?: readonly string[];
  readonly description?: string;
  readonly name: string;
}

/** Lowercase the value and return it alongside its alphanumeric word tokens. */
export function tokenizeSearchText(value: string): string[] {
  const normalized = value.toLowerCase();
  return [normalized, ...normalized.split(/[^a-z0-9]+/).filter(Boolean)];
}

/** Trim, drop leading slashes, lowercase — `/Model ` and `model` score alike. */
export function normalizeSlashSearchQuery(query: string): string {
  return query.trim().replace(/^\/+/, "").toLowerCase();
}

function scoreFields(fields: string[], query: string, offset: number): number {
  for (const field of fields) {
    if (field === query || `/${field}` === query) return offset;
  }
  for (const field of fields) {
    if (field.startsWith(query) || `/${field}`.startsWith(query)) return offset + 1;
  }
  for (const field of fields) {
    if (field.includes(query)) return offset + 2;
  }
  return Number.POSITIVE_INFINITY;
}

/** Score one item against a normalized query. Lower is better; Infinity = no match. */
export function scoreSlashMenuItem(item: SlashScoreItem, query: string): number {
  const commandFields = [item.name, ...(item.aliases ?? [])]
    .filter(Boolean)
    .flatMap(tokenizeSearchText);
  const descriptionFields = tokenizeSearchText(item.description ?? "");
  return Math.min(
    scoreFields(commandFields, query, 0),
    scoreFields(descriptionFields, query, 3),
  );
}

/** Filter and stable-sort `commands` by score (then original order). An empty
 *  query returns the list untouched so browsing keeps the caller's order. */
export function rankCommands(
  commands: ChatCommand[],
  query: string,
): { matches: ChatCommand[]; showAll: boolean } {
  const normalized = normalizeSlashSearchQuery(query);
  if (!normalized || normalized === "help") {
    return { matches: commands, showAll: normalized === "help" };
  }
  const matches = commands
    .map((command, index) => ({ command, index, score: scoreSlashMenuItem(command, normalized) }))
    .filter((entry) => entry.score !== Number.POSITIVE_INFINITY)
    .sort((a, b) => a.score - b.score || a.index - b.index)
    .map((entry) => entry.command);
  return { matches, showAll: false };
}

export default function CommandPalette({
  commands,
  activeIndex,
  showHint,
  onSelect,
  onHover,
}: {
  readonly commands: ChatCommand[];
  readonly activeIndex: number;
  readonly showHint?: boolean;
  readonly onSelect: (command: ChatCommand) => void;
  readonly onHover: (index: number) => void;
}) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listRef.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  return (
    <div
      role="listbox"
      aria-label={t("chat.palette.label")}
      className="absolute bottom-full left-0 right-0 z-30 mb-2 max-h-72 overflow-y-auto border border-outline/30 bg-surface-container-low shadow-[0_10px_30px_rgba(0,0,0,0.45)]"
    >
      <p className="sticky top-0 z-10 border-b border-outline-variant/20 bg-surface-container-low px-4 py-2 font-label-mono text-[10px] uppercase tracking-widest text-outline">
        {t("chat.palette.label")}
      </p>
      {commands.length === 0 ? (
        <p className="px-4 py-3 font-mono text-xs text-on-surface-variant/50">{t("chat.palette.noMatch")}</p>
      ) : (
        <div ref={listRef}>
          {commands.map((command, index) => {
            const active = index === activeIndex;
            return (
              <button
                key={command.name}
                type="button"
                role="option"
                aria-selected={active}
                data-active={active}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => onSelect(command)}
                onMouseEnter={() => onHover(index)}
                className={`flex w-full cursor-pointer items-center gap-3 px-4 py-2 text-left transition-colors duration-150 ${active ? "bg-primary/10" : ""}`}
              >
                <span className="shrink-0 font-mono text-sm text-primary">/{command.name}</span>
                <span className="min-w-0 flex-1 truncate text-xs text-on-surface-variant/60">{command.description}</span>
                {active && <span className="shrink-0 font-mono text-[10px] text-outline">&crarr;</span>}
              </button>
            );
          })}
        </div>
      )}
      {showHint && commands.length > 0 && (
        <p className="border-t border-outline-variant/20 px-4 py-2 font-mono text-[10px] text-on-surface-variant/50">
          {t("chat.palette.hint")}
        </p>
      )}
    </div>
  );
}
