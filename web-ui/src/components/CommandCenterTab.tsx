// Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
// command-center route semantics (MIT). Modified for Steward: maintenance
// actions over existing GetMaintenanceStatus/PruneNow RPCs; pending approvals
// from the HITL registry via ListPendingApprovals.

"use client";

import { useCallback, useEffect, useState } from "react";
import { stewardClient } from "@/lib/types";
import type { PendingApproval } from "@/lib/proto/steward";
import { useTranslation } from "@/lib/i18n/context";

export default function CommandCenterTab() {
  const { t } = useTranslation();
  const [retentionDays, setRetentionDays] = useState(0);
  const [maxCompleted, setMaxCompleted] = useState(0);
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [error, setError] = useState("");
  const [pruneSummary, setPruneSummary] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const maintenance = await stewardClient.getMaintenanceStatus({});
      setRetentionDays(maintenance.retentionDays);
      setMaxCompleted(maintenance.maxCompletedWorkflows);
      const approvalList = await stewardClient.listPendingApprovals({});
      setApprovals(approvalList.approvals);
      setError("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function prune() {
    setBusy(true);
    try {
      const response = await stewardClient.pruneNow({});
      setPruneSummary(t("commandCenter.pruned", { count: String(response.toolInvocations + response.workflows) }));
      await load();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function decide(approvalId: string, allow: boolean) {
    setBusy(true);
    try {
      await stewardClient.respondApproval({
        approvalId,
        decision: allow ? 2 : 3, // ALLOW_ALWAYS | DENY
      });
      await load();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="mx-auto w-full max-w-4xl px-4 py-8 md:px-6">
      <header className="mb-6">
        <h2 className="font-serif text-2xl text-on-surface">{t("commandCenter.title")}</h2>
        <p className="mt-1 text-sm text-on-surface-variant/60">{t("commandCenter.subtitle")}</p>
      </header>
      {error && (
        <p role="alert" className="mb-4 border border-error/40 p-3 text-sm text-error">
          {error}
        </p>
      )}
      <div className="mb-6 grid grid-cols-2 gap-4 md:grid-cols-3">
        <div className="border border-outline-variant/20 p-4">
          <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline">retention days</p>
          <p className="mt-1 font-mono text-xl text-on-surface">{retentionDays}</p>
        </div>
        <div className="border border-outline-variant/20 p-4">
          <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline">max completed</p>
          <p className="mt-1 font-mono text-xl text-on-surface">{maxCompleted}</p>
        </div>
        <div className="border border-outline-variant/20 p-4">
          <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline">pending approvals</p>
          <p className="mt-1 font-mono text-xl text-on-surface">{approvals.length}</p>
        </div>
      </div>
      {approvals.length > 0 && (
        <ul className="mb-6 space-y-2">
          {approvals.map((approval) => (
            <li key={approval.approvalId} className="flex items-center gap-3 border border-warning/40 p-3">
              <span className="min-w-0 flex-1 truncate text-sm text-on-surface">
                {approval.toolName}: {approval.summary}
              </span>
              <button type="button" disabled={busy} onClick={() => void decide(approval.approvalId, true)} className="btn-ghost">
                {t("commandCenter.prune")}
              </button>
              <button type="button" disabled={busy} onClick={() => void decide(approval.approvalId, false)} className="btn-ghost text-error">
                ✕
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="flex items-center gap-3">
        <button type="button" onClick={() => void prune()} disabled={busy} className="btn-ghost disabled:opacity-40">
          {t("commandCenter.prune")}
        </button>
        {pruneSummary && <span className="font-mono text-xs text-primary">{pruneSummary}</span>}
      </div>
    </section>
  );
}
