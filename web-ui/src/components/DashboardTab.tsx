"use client";

import { useEffect, useState } from "react";
import { stewardClient, PHASE_LABELS } from "@/lib/types";
import type { WorkflowStatus, AgentInfo } from "@/lib/types";
import { StatusPill, type StatusTone } from "./StatusPill";
import type { TabId } from "./Sidebar";
import { useTranslation } from "@/lib/i18n/context";

function agentTone(status: string): StatusTone {
  const normalized = status.toLowerCase();
  if (normalized.includes("run") || normalized.includes("active")) return "success";
  if (normalized.includes("fail") || normalized.includes("error")) return "error";
  if (normalized.includes("wait") || normalized.includes("pending")) return "warning";
  return "neutral";
}

function phaseTone(phase: number): StatusTone {
  if (phase === 10) return "success";
  if (phase === 11 || phase === 12) return "error";
  if (phase === 7) return "warning";
  return "neutral";
}

interface DashboardTabProps {
  isConnected: boolean;
  onNavigate: (tab: TabId) => void;
}

export default function DashboardTab({ isConnected, onNavigate }: DashboardTabProps) {
  const { t } = useTranslation();
  const [metrics, setMetrics] = useState({
    workflows: 0,
    agents: 0,
    nodes: 0,
  });
  const [recentWorkflows, setRecentWorkflows] = useState<WorkflowStatus[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const [wfRes, agentRes] = await Promise.all([
          stewardClient.listWorkflows({ phaseFilter: 0, limit: 5 }).catch(() => null),
          stewardClient.listAgents({}).catch(() => null),
        ]);

        const workflows = wfRes?.workflows ?? [];
        const agentsList = agentRes?.agents ?? [];

        setRecentWorkflows(workflows.slice(0, 5));
        setAgents(agentsList);
        setMetrics({
          workflows: workflows.length,
          agents: agentsList.length,
          nodes: 0,
        });

        try {
          const kg = await stewardClient.getKnowledgeGraph({ entityFilter: "", depth: 1, limit: 1 });
          setMetrics((m) => ({ ...m, nodes: kg.nodes.length }));
        } catch {
          // KG empty
        }
      } catch {
        // Daemon offline
      } finally {
        setLoading(false);
      }
    };

    load();
    const interval = setInterval(load, 10000);
    return () => clearInterval(interval);
  }, []);

  const statCards = [
    { label: "Workflows", value: metrics.workflows, color: "text-primary" },
    { label: "Agents", value: metrics.agents, color: "text-primary-container" },
    { label: "KG Nodes", value: metrics.nodes, color: "text-on-surface-variant" },
    {
      label: "Status",
      value: isConnected ? "Online" : "Offline",
      color: isConnected ? "text-primary" : "text-error",
    },
  ];

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Content area with scrolling */}
      <div className="flex-1 overflow-y-auto p-4 sm:p-6 space-y-6">
        {/* Header */}
        <div>
          <h2 className="font-headline-lg text-headline-lg text-on-surface tracking-tight">
            {t("nav.dashboard")}
          </h2>
          <p className="text-sm text-on-surface-variant/50 mt-1">
            {t("dashboard.subtitle")}
          </p>
        </div>

        {/* Stat cards — thin lined boxes, serif header, monospaced values */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {statCards.map((stat) => (
            <div
              key={stat.label}
              className="card-lift border border-outline/20 p-5 cursor-default"
            >
              <p className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50">
                {stat.label}
              </p>
              {loading ? (
                <div className="skeleton mt-2.5 h-7 w-14" />
              ) : (
                <p className={`stat-value font-mono text-2xl font-semibold mt-1.5 ${stat.color}`}>
                  {stat.value}
                </p>
              )}
              <div className="progress-gold mt-3 opacity-60">
                <div
                  className="progress-gold-fill-glow progress-gold-fill"
                  style={{ width: stat.label === "Status" ? (isConnected ? "100%" : "8%") : `${Math.min(100, Number(stat.value) * 20 + 8)}%` }}
                />
              </div>
            </div>
          ))}
        </div>

        {/* Two column layout */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Recent workflows */}
          <div className="border border-outline/20 p-5 card-ghost">
            <h3 className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface mb-3">
              Recent Workflows
            </h3>
            {loading ? (
              <div className="space-y-2">
                {[1, 2, 3].map((i) => (
                  <div key={i} className="skeleton h-10" />
                ))}
              </div>
            ) : recentWorkflows.length === 0 ? (
              <p className="text-on-surface-variant/40 text-sm">
                No workflows yet. Start one from the Workflows tab.
              </p>
            ) : (
              <div className="space-y-2">
                {recentWorkflows.map((wf) => (
                  <div
                    key={wf.workflowId}
                    className="row-interactive px-3 py-2.5 bg-surface-container-low border border-outline/10 text-sm"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="flex items-center gap-3 min-w-0">
                        <span className="text-on-surface truncate max-w-[180px]">
                          {wf.title}
                        </span>
                        <StatusPill tone={phaseTone(wf.phase)}>
                          {PHASE_LABELS[wf.phase] ?? "Unknown"}
                        </StatusPill>
                      </div>
                      <span className="stat-value text-outline text-xs font-mono shrink-0">
                        {Math.min(100, Math.max(0, Math.round(wf.overallProgress)))}%
                      </span>
                    </div>
                    <div className="progress-gold mt-2">
                      <div
                        className="progress-gold-fill"
                        style={{ width: `${Math.min(100, Math.max(0, Math.round(wf.overallProgress)))}%` }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Active agents */}
          <div className="border border-outline/20 p-5 card-ghost">
            <h3 className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface mb-3">
              Active Agents
            </h3>
            {loading ? (
              <div className="space-y-2">
                {[1, 2, 3].map((i) => (
                  <div key={i} className="skeleton h-10" />
                ))}
              </div>
            ) : agents.length === 0 ? (
              <p className="text-on-surface-variant/40 text-sm">
                No agents registered yet.
              </p>
            ) : (
              <div className="space-y-2">
                {agents.map((agent) => (
                  <div
                    key={agent.agentId}
                    className="row-interactive flex items-center justify-between px-3 py-2.5 bg-surface-container-low border border-outline/10 text-sm"
                  >
                    <div className="flex items-center gap-3">
                      <span className="text-on-surface">{agent.name}</span>
                      <span className="text-on-surface-variant/40 text-xs">{agent.role}</span>
                    </div>
                    <StatusPill tone={agentTone(agent.status)}>{agent.status}</StatusPill>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Quick actions */}
        <div className="border border-outline/20 p-5 card-ghost">
          <h3 className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface mb-3">
            Quick Actions
          </h3>
          <div className="flex flex-wrap gap-3">
            <button
              onClick={() => onNavigate("workflows")}
              className="btn-ghost"
            >
              New Workflow
            </button>
            <button
              onClick={() => onNavigate("knowledge")}
              className="btn-ghost"
            >
              Explore Graph
            </button>
            <button
              onClick={() => onNavigate("agents")}
              className="btn-ghost"
            >
              View Agents
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
