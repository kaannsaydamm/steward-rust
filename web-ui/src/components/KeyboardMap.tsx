"use client";

import { useTranslation } from "@/lib/i18n/context";

type I18nKey = Parameters<ReturnType<typeof useTranslation>["t"]>[0];

const SHORTCUTS: { keys: string; action: I18nKey }[] = [
  { keys: "Enter", action: "keyboard.send" },
  { keys: "Shift+Enter / Ctrl+J", action: "keyboard.newline" },
  { keys: "↑ / ↓", action: "keyboard.history" },
  { keys: "/", action: "keyboard.commands" },
  { keys: "Tab", action: "keyboard.complete" },
  { keys: "Esc", action: "keyboard.paletteClose" },
  { keys: "Esc Esc", action: "keyboard.clearDraft" },
  { keys: "Ctrl+X", action: "keyboard.switchSession" },
  { keys: "Ctrl+S", action: "keyboard.stashDraft" },
  { keys: "Alt+1…9", action: "keyboard.tabJump" },
  { keys: "?", action: "keyboard.thisHelp" },
];

export default function KeyboardMap({ open, onClose }: { readonly open: boolean; readonly onClose: () => void }) {
  const { t } = useTranslation();
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-[55] flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("keyboard.title")}
        className="composer-surface w-full max-w-lg p-6 shadow-[0_20px_60px_rgba(0,0,0,0.6)]"
        onClick={(event) => event.stopPropagation()}
      >
        <h3 className="mb-4 font-serif text-lg text-on-surface">{t("keyboard.title")}</h3>
        <dl className="space-y-2">
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.keys} className="flex items-center justify-between gap-4">
              <dt className="shrink-0 border border-outline-variant/40 px-2 py-0.5 font-mono text-[11px] text-primary">
                {shortcut.keys}
              </dt>
              <dd className="text-right text-xs text-on-surface-variant/70">{t(shortcut.action)}</dd>
            </div>
          ))}
        </dl>
        <p className="mt-4 font-mono text-[10px] text-outline">Esc to close</p>
      </div>
    </div>
  );
}
