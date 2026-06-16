"use client";

import { useEffect, useState } from "react";
import { stewardClient, PHASE_LABELS } from "@/lib/types";
import type { WorkflowStatus, AgentInfo } from "@/lib/types";
import Terminal from "./Terminal";

export default function DashboardTab() {
  const [metrics, setMetrics] = useState({
    workflows: 0,
    agents: 0,
    nodes: 0,
    uptime: "--",
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
          uptime: "Active",
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
    { label: "Status", value: metrics.uptime, color: "text-primary" },
  ];

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Content area with scrolling */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* Header */}
        <div>
          <h2 className="font-headline-lg text-headline-lg text-on-surface tracking-tight">
            Dashboard
          </h2>
          <p className="text-sm text-on-surface-variant/50 mt-1">
            System overview and real-time metrics
          </p>
        </div>

        {/* Stat cards — thin lined boxes, serif header, monospaced values */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {statCards.map((stat) => (
            <div
              key={stat.label}
              className="border border-outline/20 p-5 card-ghost"
            >
              <p className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50">
                {stat.label}
              </p>
              <p className={`font-mono text-2xl font-semibold mt-1.5 ${stat.color}`}>
                {loading ? (
                  <span className="animate-pulse text-outline-variant">--</span>
                ) : (
                  stat.value
                )}
              </p>
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
                  <div key={i} className="h-10 bg-surface-container-high animate-pulse" />
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
                    className="flex items-center justify-between px-3 py-2.5 bg-surface-container-low border border-outline/10 text-sm"
                  >
                    <div className="flex items-center gap-3 min-w-0">
                      <span className="text-on-surface truncate max-w-[180px]">
                        {wf.title}
                      </span>
                      <span className="chip-bracket text-on-surface-variant/50 shrink-0">
                        {PHASE_LABELS[wf.phase] ?? "Unknown"}
                      </span>
                    </div>
                    <span className="text-outline text-xs font-mono shrink-0">
                      {Math.round(wf.overallProgress * 100)}%
                    </span>
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
                  <div key={i} className="h-10 bg-surface-container-high animate-pulse" />
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
                    className="flex items-center justify-between px-3 py-2.5 bg-surface-container-low border border-outline/10 text-sm"
                  >
                    <div className="flex items-center gap-3">
                      <span className="w-2 h-2 rounded-full bg-primary" />
                      <span className="text-on-surface">{agent.name}</span>
                    </div>
                    <span className="text-on-surface-variant/50 text-xs">{agent.role}</span>
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
              onClick={() => {
                const tab = document.querySelector('[data-tab="workflows"]') as HTMLElement;
                tab?.click();
              }}
              className="btn-ghost"
            >
              New Workflow
            </button>
            <button
              onClick={() => {
                const tab = document.querySelector('[data-tab="knowledge"]') as HTMLElement;
                tab?.click();
              }}
              className="btn-ghost"
            >
              Explore Graph
            </button>
            <button
              onClick={() => {
                const tab = document.querySelector('[data-tab="agents"]') as HTMLElement;
                tab?.click();
              }}
              className="btn-ghost"
            >
              View Agents
            </button>
          </div>
        </div>
      </div>

      {/* Terminal at bottom */}
      <div className="h-72 shrink-0 border-t border-outline-variant/20">
        <Terminal />
      </div>
    </div>
  );
}
