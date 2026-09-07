// Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
// settings route semantics (MIT). Modified for Steward: daemon configuration
// read/write over existing GetSecuritySettings/SaveSecuritySettings RPCs plus
// retention status from GetMaintenanceStatus.

"use client";

import { useCallback, useEffect, useState } from "react";
import { stewardClient } from "@/lib/types";
import type { SecuritySettingsInfo } from "@/lib/proto/steward";
import { useTranslation } from "@/lib/i18n/context";

export default function SettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<SecuritySettingsInfo | null>(null);
  const [allowlistText, setAllowlistText] = useState("");
  const [retentionDays, setRetentionDays] = useState(0);
  const [maxCompleted, setMaxCompleted] = useState(0);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    try {
      const security = await stewardClient.getSecuritySettings({});
      setSettings(security);
      setAllowlistText(security.processExecAllowlist.join("\n"));
      const maintenance = await stewardClient.getMaintenanceStatus({});
      setRetentionDays(maintenance.retentionDays);
      setMaxCompleted(maintenance.maxCompletedWorkflows);
      setError("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    if (!settings) return;
    try {
      const next: SecuritySettingsInfo = {
        ...settings,
        processExecAllowlist: allowlistText
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
      };
      await stewardClient.saveSecuritySettings(next);
      setSaved(true);
      setError("");
      window.setTimeout(() => setSaved(false), 2_000);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <section className="mx-auto w-full max-w-3xl px-4 py-8 md:px-6">
      <header className="mb-6">
        <h2 className="font-serif text-2xl text-on-surface">{t("settings.title")}</h2>
        <p className="mt-1 text-sm text-on-surface-variant/60">{t("settings.subtitle")}</p>
      </header>
      {error && (
        <p role="alert" className="mb-4 border border-error/40 p-3 text-sm text-error">
          {error}
        </p>
      )}
      {settings && (
        <div className="space-y-5 border border-outline-variant/20 p-5">
          <label className="block">
            <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">
              process.exec allowlist
            </span>
            <textarea
              rows={5}
              value={allowlistText}
              onChange={(event) => setAllowlistText(event.target.value)}
              className="w-full resize-y border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none focus:border-primary/60"
            />
          </label>
          <div className="grid grid-cols-2 gap-4">
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">
                retention days
              </span>
              <input
                type="number"
                min={1}
                value={retentionDays}
                disabled
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface-variant/60 outline-none"
              />
            </label>
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">
                max completed workflows
              </span>
              <input
                type="number"
                min={0}
                value={maxCompleted}
                disabled
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface-variant/60 outline-none"
              />
            </label>
          </div>
          <button type="button" onClick={() => void save()} className="btn-ghost">
            {t("chat.send")}
            {saved && <span className="ml-2 text-primary">✓</span>}
          </button>
        </div>
      )}
    </section>
  );
}
