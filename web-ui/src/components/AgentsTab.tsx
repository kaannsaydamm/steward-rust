"use client";

import { useEffect, useState, useCallback } from "react";
import { stewardClient, collectStream } from "@/lib/types";
import type { AgentInfo, AgentLogEntry } from "@/lib/types";

export default function AgentsTab() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<AgentInfo | null>(null);
  const [logs, setLogs] = useState<AgentLogEntry[]>([]);
  const [logLoading, setLogLoading] = useState(false);

  const loadAgents = useCallback(async () => {
    try {
      const res = await stewardClient.listAgents({});
      setAgents(res.agents);
    } catch {
      // offline
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => {
      void loadAgents();
    }, 0);
    const interval = setInterval(loadAgents, 5000);
    return () => {
      clearTimeout(initialLoad);
      clearInterval(interval);
    };
  }, [loadAgents]);

  // Load logs for selected agent
  const loadLogs = useCallback(async (agentId: string) => {
    setLogLoading(true);
    try {
      const stream = await stewardClient.getAgentLog({
        workflowId: "",
        agentId,
      });
      const entries = await collectStream(stream);
      setLogs(entries);
    } catch {
      setLogs([]);
    } finally {
      setLogLoading(false);
    }
  }, []);

  const handleSelectAgent = async (agent: AgentInfo) => {
    setSelectedAgent(agent);
    await loadLogs(agent.agentId);
  };

  const statusColor = (status: string) => {
    switch (status) {
      case "idle":
        return "#99907C";
      case "working":
        return "#D4AF37";
      case "error":
        return "#FFB4AB";
      case "completed":
        return "#F2CA50";
      default:
        return "#4D4635";
    }
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto p-6">
        {/* Header */}
        <div className="mb-5">
          <h2 className="font-headline-lg text-headline-lg text-on-surface tracking-tight">
            Agents
          </h2>
          <p className="text-sm text-on-surface-variant/50 mt-1">
            Monitor agent activities and logs
          </p>
        </div>

        {loading ? (
          <div className="text-center text-sm text-outline font-mono py-12">Loading agents...</div>
        ) : agents.length === 0 ? (
          <div className="text-center text-sm text-on-surface-variant/50 font-mono py-12">
            No agents registered yet. Agents appear when workflows create them.
          </div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            {/* Agent list */}
            <div className="lg:col-span-1 space-y-2">
              <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline mb-2">
                [{agents.length} Agent{agents.length !== 1 ? "s" : ""}]
              </p>
              {agents.map((agent) => (
                <button
                  key={agent.agentId}
                  onClick={() => handleSelectAgent(agent)}
                  className={`w-full text-left border p-4 card-ghost transition-all ${
                    selectedAgent?.agentId === agent.agentId
                      ? "border-primary/40"
                      : "border-outline/20"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <span
                      className="inline-block w-2.5 h-2.5 rounded-full shrink-0"
                      style={{ backgroundColor: statusColor(agent.status) }}
                    />
                    <div className="min-w-0">
                      <p className="text-sm text-on-surface font-medium truncate">
                        {agent.name}
                      </p>
                      <p className="font-label-mono text-[10px] text-on-surface-variant/50 mt-0.5">{agent.role}</p>
                    </div>
                  </div>
                  <div className="mt-2">
                    <div className="flex items-center justify-between font-label-mono text-[10px] uppercase tracking-wider">
                      <span className="text-outline">{agent.status}</span>
                      <span className="text-outline font-mono">
                        {Math.round(agent.progress * 100)}%
                      </span>
                    </div>
                    <div className="progress-gold w-full mt-1">
                      <div
                        className="progress-gold-fill"
                        style={{ width: `${Math.round(agent.progress * 100)}%` }}
                      />
                    </div>
                  </div>
                  {agent.currentTask && (
                    <p className="font-label-mono text-[10px] text-on-surface-variant/40 mt-2 truncate">
                      {agent.currentTask}
                    </p>
                  )}
                </button>
              ))}
            </div>

            {/* Log detail */}
            <div className="lg:col-span-2">
              {selectedAgent ? (
                <div className="border border-outline/20 p-5 card-ghost">
                  <div className="flex items-center gap-3 mb-4">
                    <span
                      className="inline-block w-3 h-3 rounded-full"
                      style={{ backgroundColor: statusColor(selectedAgent.status) }}
                    />
                    <div>
                      <p className="text-sm text-on-surface font-medium">
                        {selectedAgent.name}
                      </p>
                      <p className="font-label-mono text-[10px] text-on-surface-variant/50">
                        {selectedAgent.role} · {selectedAgent.agentId.slice(0, 8)}...
                      </p>
                    </div>
                  </div>

                  {logLoading ? (
                    <div className="text-center text-sm text-outline font-mono py-8">
                      Loading logs...
                    </div>
                  ) : logs.length === 0 ? (
                    <div className="text-center text-sm text-on-surface-variant/50 font-mono py-8">
                      No log entries yet.
                    </div>
                  ) : (
                    <div className="space-y-1 max-h-[500px] overflow-y-auto">
                      {logs.map((entry, i) => {
                        const levelColor =
                          entry.level === "error"
                            ? "text-error"
                            : entry.level === "warn"
                            ? "text-primary-container"
                            : "text-on-surface/70";
                        return (
                          <div
                            key={i}
                            className="flex items-start gap-2 font-mono text-xs py-1.5 border-b border-outline-variant/10"
                          >
                            <span className="text-outline font-mono shrink-0">
                              [{new Date(entry.timestamp * 1000).toLocaleTimeString()}]
                            </span>
                            <span className={`shrink-0 uppercase ${levelColor}`}>
                              {entry.level}
                            </span>
                            <span className="text-on-surface/80 break-words">
                              {entry.message}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              ) : (
                <div className="flex items-center justify-center h-full min-h-[300px] text-sm text-outline font-mono border border-outline/20 p-5">
                  Select an agent to view details
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
