"use client";

import { useEffect, useState, useCallback } from "react";
import {
  stewardClient,
  PHASE_LABELS,
  PHASE_COLORS,
  MODE_LABELS,
} from "@/lib/types";
import type { WorkflowStatus, WorkflowEvent } from "@/lib/types";
import WorkflowBuilder from "@/components/WorkflowBuilder";

interface WorkflowsTabProps {
  initiallyOpenCreator?: boolean;
}

export default function WorkflowsTab({
  initiallyOpenCreator = false,
}: WorkflowsTabProps) {
  const [workflows, setWorkflows] = useState<WorkflowStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [createOpen, setCreateOpen] = useState(initiallyOpenCreator);
  const [selectedWf, setSelectedWf] = useState<WorkflowStatus | null>(null);
  const [detailEvents, setDetailEvents] = useState<WorkflowEvent[]>([]);

  const loadWorkflows = useCallback(async () => {
    try {
      const res = await stewardClient.listWorkflows({ phaseFilter: 0, limit: 50 });
      setWorkflows(res.workflows);
    } catch {
      // offline
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => {
      void loadWorkflows();
    }, 0);
    const interval = setInterval(loadWorkflows, 5000);
    return () => {
      clearTimeout(initialLoad);
      clearInterval(interval);
    };
  }, [loadWorkflows]);

  // Cancel
  const handleCancel = async (workflowId: string) => {
    try {
      await stewardClient.cancelWorkflow({ workflowId, reason: "Cancelled by user" });
      await loadWorkflows();
    } catch (err: unknown) {
      console.error("Cancel failed:", err);
    }
  };

  // Approve
  const handleApprove = async (workflowId: string, mode: number, approved: boolean, feedback: string) => {
    try {
      await stewardClient.approvePlan({ workflowId, mode, approved, feedback });
      await loadWorkflows();
    } catch (err: unknown) {
      console.error("Approve failed:", err);
    }
  };

  // Show detail
  const handleSelect = async (wf: WorkflowStatus) => {
    setSelectedWf(wf);
    setDetailEvents(wf.recentEvents || []);
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto p-4 sm:p-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-5">
          <div>
            <h2 className="font-headline-lg text-headline-lg text-on-surface tracking-tight">
              Workflows
            </h2>
            <p className="text-sm text-on-surface-variant/50 mt-1">
              Manage agent workflow pipelines
            </p>
          </div>
          <button
            onClick={() => setCreateOpen(!createOpen)}
            className="btn-ghost"
          >
            {createOpen ? "[Show Runs]" : "[Open Builder]"}
          </button>
        </div>

        {createOpen && (
          <div className="mb-8"><WorkflowBuilder onRunFinished={loadWorkflows} /></div>
        )}

        {!createOpen && (loading ? (
          <div className="text-center text-sm text-outline font-mono py-12">Loading workflows...</div>
        ) : workflows.length === 0 ? (
          <div className="text-center text-sm text-on-surface-variant/50 font-mono py-12">
            No workflows yet. Click <span className="text-primary">[+ New Workflow]</span> to start.
          </div>
        ) : (
          <div className="space-y-2">
            {workflows.map((wf) => (
              <div
                key={wf.workflowId}
                className={`border p-4 card-ghost cursor-pointer ${
                  selectedWf?.workflowId === wf.workflowId
                    ? "border-primary/40"
                    : "border-outline/20"
                }`}
                onClick={() => handleSelect(wf)}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3 min-w-0">
                    <div>
                      <p className="text-sm text-on-surface font-medium">{wf.title}</p>
                      <div className="flex items-center gap-2 mt-1">
                        <span className={`chip-bracket ${PHASE_COLORS[wf.phase] || "text-on-surface-variant/50"}`}>
                          {PHASE_LABELS[wf.phase] || "Unknown"}
                        </span>
                        <span className="text-outline text-xs">·</span>
                        <span className="chip-bracket text-outline">
                          {MODE_LABELS[wf.mode] || "Unknown"}
                        </span>
                        {wf.requiresApproval && (
                          <>
                            <span className="text-outline text-xs">·</span>
                            <span className="chip-bracket text-primary">
                              Awaiting approval
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 shrink-0">
                    <div className="text-right">
                      <span className="text-sm font-mono text-on-surface">
                        {Math.min(100, Math.max(0, Math.round(wf.overallProgress)))}%
                      </span>
                      <div className="progress-gold w-20 mt-1">
                        <div
                          className="progress-gold-fill"
                          style={{ width: `${Math.min(100, Math.max(0, Math.round(wf.overallProgress)))}%` }}
                        />
                      </div>
                    </div>
                    {wf.workflowId !== selectedWf?.workflowId && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleCancel(wf.workflowId);
                        }}
                        className="font-label-mono text-[10px] uppercase tracking-wider text-outline hover:text-error transition-colors"
                      >
                        Cancel
                      </button>
                    )}
                  </div>
                </div>

                {/* Detail */}
                {selectedWf?.workflowId === wf.workflowId && (
                  <div className="mt-4 pt-4 border-t border-outline-variant/20">
                    <p className="text-sm text-on-surface-variant/50 mb-2 font-mono">
                      {wf.statusMessage || "Running..."}
                    </p>

                    {/* Approval UI */}
                    {wf.requiresApproval && wf.pendingApproval && (
                      <div className="border border-primary/20 p-4 mb-3 bg-primary/5">
                        <p className="text-sm text-primary font-mono font-medium mb-2">
                          [PLAN_APPROVAL_REQUIRED]
                        </p>
                        <p className="font-label-mono text-[11px] text-on-surface-variant/60 mb-1">
                          {wf.pendingApproval.planSummary || wf.pendingApproval.description}
                        </p>
                        <div className="flex gap-2 mt-3">
                          <button
                            onClick={() => handleApprove(wf.workflowId, wf.mode, true, "")}
                            className="btn-ghost"
                          >
                            Approve Plan
                          </button>
                          <button
                            onClick={() => handleApprove(wf.workflowId, wf.mode, false, "Rejected")}
                            className="btn-ghost text-error border-error/30 hover:border-error/50"
                          >
                            Reject
                          </button>
                        </div>
                      </div>
                    )}

                    {/* Events */}
                    {detailEvents.length > 0 && (
                      <div className="space-y-1 max-h-40 overflow-y-auto">
                        <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline mb-1">
                          Events
                        </p>
                        {detailEvents.map((ev, i) => (
                          <div
                            key={i}
                            className="flex items-start gap-2 font-mono text-xs py-1 border-b border-outline-variant/10 last:border-0"
                          >
                            <span className={`shrink-0 ${
                              ev.phase >= 10 ? "text-primary" :
                              ev.phase >= 7 ? "text-primary-container" :
                              "text-outline"
                            }`}>
                              [{PHASE_LABELS[ev.phase] || ev.phase}]
                            </span>
                            <span className="text-on-surface/70">{ev.message}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
