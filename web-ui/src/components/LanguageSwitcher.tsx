"use client";

import { useTranslation } from "@/lib/i18n/context";
import { LANGUAGES } from "@/lib/i18n/languages";

export default function LanguageSwitcher() {
  const { language, setLanguage, t } = useTranslation();

  return (
    <label className="block">
      <span className="sr-only">{t("sidebar.language")}</span>
      <select
        value={language}
        onChange={(event) => setLanguage(event.target.value)}
        aria-label={t("sidebar.language")}
        className="w-full border border-outline-variant/30 bg-transparent px-2 py-1.5 font-label-mono text-[10px] uppercase tracking-wider text-on-surface-variant/70"
      >
        {LANGUAGES.map((entry) => (
          <option key={entry.code} value={entry.code} className="bg-background text-on-surface">
            {entry.nativeName}
          </option>
        ))}
      </select>
    </label>
  );
}
