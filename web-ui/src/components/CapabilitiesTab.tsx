"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient, timeAgo } from "@/lib/types";
import type {
  MaintenanceStatus,
  McpAdapterInfo,
  McpCatalogEntry,
  SkillInfo,
  ToolInfo,
  ToolInvocationInfo,
} from "@/lib/types";

function ProcessAllowlist() {
  const [allowlist, setAllowlist] = useState<string[]>([]);
  const [program, setProgram] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const settings = await stewardClient.getSecuritySettings({});
    setAllowlist(settings.processExecAllowlist);
  }, []);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => {
      void load();
    }, 0);
    return () => window.clearTimeout(initialLoad);
  }, [load]);

  const save = async (next: string[]) => {
    setBusy(true);
    try {
      await stewardClient.saveSecuritySettings({ processExecAllowlist: next });
      setAllowlist(next);
    } finally {
      setBusy(false);
    }
  };

  const addProgram = async (event: FormEvent) => {
    event.preventDefault();
    const trimmed = program.trim();
    if (!trimmed || allowlist.includes(trimmed)) return;
    await save([...allowlist, trimmed].sort());
    setProgram("");
  };

  return (
    <Panel title={`Process.exec allowlist / ${allowlist.length}`}>
      <p className="mb-3 text-sm text-outline">
        Empty allowlist permits any command once process.exec is enabled and approved. Add
        program names to restrict execution to that set.
      </p>
      <form onSubmit={addProgram} className="mb-4 flex gap-2 border-b border-outline-variant/10 pb-4">
        <input
          value={program}
          onChange={(event) => setProgram(event.target.value)}
          placeholder="e.g. git"
          className="input-ledger flex-1"
        />
        <button type="submit" disabled={busy || !program.trim()} className="btn-ghost">
          Allow
        </button>
      </form>
      {allowlist.map((entry) => (
        <div key={entry} className="flex items-center justify-between gap-3 py-2 border-b border-outline-variant/10">
          <span className="font-label-mono text-xs text-on-surface">{entry}</span>
          <button
            type="button"
            disabled={busy}
            onClick={() => void save(allowlist.filter((item) => item !== entry))}
            className="border border-error/30 px-3 py-1 font-label-mono text-[9px] uppercase text-error disabled:opacity-40"
          >
            Remove
          </button>
        </div>
      ))}
      {allowlist.length === 0 && <Empty text="No restrictions — process.exec permits any approved command" />}
    </Panel>
  );
}

type ToolFilter = "all" | "enabled" | "disabled" | "low" | "medium" | "high";

const RISK_LABELS: Record<number, string> = { 0: "unspecified", 1: "low", 2: "medium", 3: "high" };
const RISK_FILTERS: { id: ToolFilter; label: string; risk?: number }[] = [
  { id: "low", label: "Low risk", risk: 1 },
  { id: "medium", label: "Medium risk", risk: 2 },
  { id: "high", label: "High risk", risk: 3 },
];

function matchesFilter(tool: ToolInfo, filter: ToolFilter): boolean {
  if (filter === "all") return true;
  if (filter === "enabled") return tool.enabled;
  if (filter === "disabled") return !tool.enabled;
  const risk = RISK_FILTERS.find((entry) => entry.id === filter)?.risk;
  return risk !== undefined && tool.risk === risk;
}

export default function CapabilitiesTab() {
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [adapters, setAdapters] = useState<McpAdapterInfo[]>([]);
  const [catalog, setCatalog] = useState<McpCatalogEntry[]>([]);
  const [audit, setAudit] = useState<ToolInvocationInfo[]>([]);
  const [maintenance, setMaintenance] = useState<MaintenanceStatus | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [toolFilter, setToolFilter] = useState<ToolFilter>("all");
  const [selectedToolId, setSelectedToolId] = useState("");
  const [adapterId, setAdapterId] = useState("");
  const [adapterName, setAdapterName] = useState("");
  const [adapterCommand, setAdapterCommand] = useState("");
  const [adapterArgs, setAdapterArgs] = useState("");

  const load = useCallback(async () => {
    try {
      const [toolResult, skillResult, adapterResult, catalogResult, auditResult, maintenanceResult] =
        await Promise.all([
          stewardClient.listTools({ includeDisabled: true }),
          stewardClient.listSkills({ includeDisabled: true }),
          stewardClient.listMcpAdapters({}),
          stewardClient.listMcpCatalog({}),
          stewardClient.listToolInvocations({ limit: 20 }),
          stewardClient.getMaintenanceStatus({}),
        ]);
      setTools(toolResult.tools);
      setSkills(skillResult.skills);
      setAdapters(adapterResult.adapters);
      setCatalog(catalogResult.entries);
      setAudit(auditResult.invocations);
      setMaintenance(maintenanceResult);
      setSelectedToolId((current) =>
        current && toolResult.tools.some((tool) => tool.toolId === current)
          ? current
          : toolResult.tools[0]?.toolId ?? ""
      );
      setError("");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    }
  }, []);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => {
      void load();
    }, 0);
    return () => window.clearTimeout(initialLoad);
  }, [load]);

  const setAdapterState = async (adapter: McpAdapterInfo) => {
    setBusy(adapter.adapterId);
    try {
      if (adapter.status === "running") {
        await stewardClient.stopMcpAdapter({ adapterId: adapter.adapterId });
      } else {
        await stewardClient.startMcpAdapter({ adapterId: adapter.adapterId });
      }
      await load();
    } finally {
      setBusy("");
    }
  };

  const removeAdapter = async (adapter: McpAdapterInfo) => {
    setBusy(adapter.adapterId);
    try {
      await stewardClient.removeMcpAdapter({ adapterId: adapter.adapterId });
      await load();
    } finally {
      setBusy("");
    }
  };

  const registerAdapter = async (event: FormEvent) => {
    event.preventDefault();
    setBusy("register");
    try {
      await stewardClient.registerMcpAdapter({
        adapterId: adapterId.trim(),
        name: adapterName.trim(),
        command: adapterCommand.trim(),
        arguments: adapterArgs.trim() ? adapterArgs.trim().split(/\s+/) : [],
        cwd: "",
      });
      setAdapterId("");
      setAdapterName("");
      setAdapterCommand("");
      setAdapterArgs("");
      await load();
      setError("");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    } finally {
      setBusy("");
    }
  };

  const applyCatalogEntry = (entry: McpCatalogEntry) => {
    setAdapterId(entry.catalogId);
    setAdapterName(entry.name);
    setAdapterCommand(entry.command);
    setAdapterArgs(entry.args.join(" "));
  };

  const toggleTool = async (tool: ToolInfo) => {
    setBusy(tool.toolId);
    try {
      await stewardClient.setToolEnabled({ toolId: tool.toolId, enabled: !tool.enabled });
      await load();
    } finally {
      setBusy("");
    }
  };

  const prune = async () => {
    setBusy("prune");
    try {
      await stewardClient.pruneNow({});
      await load();
    } finally {
      setBusy("");
    }
  };

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-6">
      <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-4 mb-6">
        <div>
          <h2 className="font-headline-lg text-headline-lg text-on-surface">Capabilities</h2>
          <p className="text-sm text-on-surface-variant/50 mt-1">
            Governed tools, signed skills, MCP adapters, and invocation audit
          </p>
        </div>
        <button
          type="button"
          onClick={prune}
          disabled={busy !== ""}
          className="border border-primary/40 px-4 py-2 font-label-mono text-[10px] uppercase tracking-widest text-primary disabled:opacity-40"
        >
          Run retention
        </button>
      </div>

      {error && <div className="border border-error/40 p-3 text-sm text-error mb-5">{error}</div>}
      {maintenance && (
        <p className="font-label-mono text-[10px] uppercase tracking-widest text-outline mb-5">
          [Retention {maintenance.retentionDays} days / {maintenance.maxCompletedWorkflows} completed workflows]
        </p>
      )}

      <section className="mb-6 border border-outline/20 card-ghost">
        <h3 className="border-b border-outline-variant/10 p-4 font-label-mono text-[10px] uppercase tracking-widest text-primary">
          [Tools / {tools.length}]
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-[140px_1fr_1fr]">
          <nav className="border-b border-outline-variant/10 p-3 md:border-b-0 md:border-r">
            {(["all", "enabled", "disabled"] as ToolFilter[]).map((id) => (
              <FilterButton
                key={id}
                active={toolFilter === id}
                label={id === "all" ? `All (${tools.length})` : `${id[0].toUpperCase()}${id.slice(1)} (${tools.filter((tool) => matchesFilter(tool, id)).length})`}
                onClick={() => setToolFilter(id)}
              />
            ))}
            <div className="my-2 h-px bg-outline-variant/10" />
            {RISK_FILTERS.map((entry) => (
              <FilterButton
                key={entry.id}
                active={toolFilter === entry.id}
                label={`${entry.label} (${tools.filter((tool) => matchesFilter(tool, entry.id)).length})`}
                onClick={() => setToolFilter(entry.id)}
              />
            ))}
          </nav>
          <div className="max-h-96 overflow-y-auto border-b border-outline-variant/10 md:border-b-0 md:border-r">
            {tools.filter((tool) => matchesFilter(tool, toolFilter)).map((tool) => (
              <button
                key={tool.toolId}
                type="button"
                onClick={() => setSelectedToolId(tool.toolId)}
                className={`flex w-full items-center justify-between gap-3 border-b border-outline-variant/10 px-4 py-3 text-left ${
                  selectedToolId === tool.toolId ? "bg-primary/5" : "hover:bg-surface-container"
                }`}
              >
                <div className="min-w-0">
                  <p className="truncate text-sm text-on-surface">{tool.name}</p>
                  <p className="truncate font-label-mono text-[10px] text-outline">{tool.toolId}</p>
                </div>
                <span className={`shrink-0 font-label-mono text-[9px] uppercase ${tool.enabled ? "text-primary" : "text-outline"}`}>
                  {tool.enabled ? "on" : "off"}
                </span>
              </button>
            ))}
            {tools.filter((tool) => matchesFilter(tool, toolFilter)).length === 0 && (
              <Empty text="No tools match this filter" />
            )}
          </div>
          <div className="p-5">
            {(() => {
              const tool = tools.find((item) => item.toolId === selectedToolId);
              if (!tool) return <Empty text="Select a tool to see details" />;
              return (
                <div className="flex h-full flex-col gap-3">
                  <div>
                    <p className="text-sm text-on-surface">{tool.name}</p>
                    <p className="font-label-mono text-[10px] text-outline">{tool.toolId}</p>
                  </div>
                  <p className="text-sm text-outline">{tool.description || "No description provided."}</p>
                  <dl className="grid grid-cols-2 gap-2 font-label-mono text-[10px] uppercase tracking-widest text-outline">
                    <dt>Risk</dt>
                    <dd className="text-on-surface">{RISK_LABELS[tool.risk] ?? "unknown"}</dd>
                    <dt>Approval</dt>
                    <dd className="text-on-surface">{tool.requiresApproval ? "required" : "not required"}</dd>
                  </dl>
                  <button
                    type="button"
                    disabled={busy !== ""}
                    onClick={() => void toggleTool(tool)}
                    className="mt-auto border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-primary disabled:opacity-40"
                  >
                    {tool.enabled ? "Disable" : "Enable"}
                  </button>
                </div>
              );
            })()}
          </div>
        </div>
      </section>

      <section className="mb-6">
        <Panel title={`Signed skills / ${skills.length}`}>
          {skills.map((skill) => (
            <Row key={skill.skillId} title={skill.name} detail={`${skill.skillId} · ${skill.toolIds.length} tools`} state={skill.signed ? "signed" : "unsigned"} />
          ))}
          {skills.length === 0 && <Empty text="No skills installed" />}
        </Panel>
      </section>

      <section className="mb-6">
        <Panel title={`Connector marketplace / ${catalog.length}`}>
          <p className="mb-3 text-sm text-outline">
            Curated official MCP reference servers. Pick one to prefill the register form below,
            then give it a local instance id and register it.
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3">
            {catalog.map((entry) => (
              <div key={entry.catalogId} className="flex flex-col gap-2 border border-outline/20 p-3">
                <div>
                  <p className="text-sm text-on-surface">{entry.name}</p>
                  <p className="font-label-mono text-[9px] uppercase tracking-widest text-outline">
                    {entry.publisher}
                  </p>
                </div>
                <p className="flex-1 text-xs text-outline">{entry.description}</p>
                <p className="truncate font-label-mono text-[10px] text-outline">
                  {entry.command} {entry.args.join(" ")}
                </p>
                <button
                  type="button"
                  onClick={() => applyCatalogEntry(entry)}
                  className="self-start border border-primary/40 px-3 py-1 font-label-mono text-[9px] uppercase text-primary"
                >
                  Use
                </button>
              </div>
            ))}
          </div>
          {catalog.length === 0 && <Empty text="No catalog entries available" />}
        </Panel>
      </section>

      <section className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <Panel title={`MCP adapters / ${adapters.length}`}>
          <form onSubmit={registerAdapter} className="mb-4 grid grid-cols-2 gap-2 border-b border-outline-variant/10 pb-4">
            <input value={adapterId} onChange={(event) => setAdapterId(event.target.value)} placeholder="adapter id" className="input-ledger" />
            <input value={adapterName} onChange={(event) => setAdapterName(event.target.value)} placeholder="name" className="input-ledger" />
            <input value={adapterCommand} onChange={(event) => setAdapterCommand(event.target.value)} placeholder="command" className="input-ledger" />
            <input value={adapterArgs} onChange={(event) => setAdapterArgs(event.target.value)} placeholder="args (space separated)" className="input-ledger" />
            <button
              type="submit"
              disabled={busy !== "" || !adapterId.trim() || !adapterName.trim() || !adapterCommand.trim()}
              className="btn-ghost col-span-2"
            >
              Register adapter
            </button>
          </form>
          {adapters.map((adapter) => (
            <div key={adapter.adapterId} className="flex items-center gap-3 py-3 border-b border-outline-variant/10">
              <div className="min-w-0 flex-1">
                <p className="text-sm text-on-surface truncate">{adapter.name}</p>
                <p className="font-label-mono text-[10px] text-outline truncate">{adapter.adapterId} · {adapter.toolCount} tools</p>
              </div>
              <button type="button" disabled={busy !== ""} onClick={() => void setAdapterState(adapter)} className="border border-outline/30 px-3 py-1 font-label-mono text-[10px] uppercase text-primary disabled:opacity-40">
                {adapter.status === "running" ? "Stop" : "Start"}
              </button>
              <button type="button" disabled={busy !== ""} onClick={() => void removeAdapter(adapter)} className="border border-error/30 px-3 py-1 font-label-mono text-[10px] uppercase text-error disabled:opacity-40">
                Remove
              </button>
            </div>
          ))}
          {adapters.length === 0 && <Empty text="No MCP adapters registered" />}
        </Panel>
        <Panel title={`Invocation audit / ${audit.length}`}>
          {audit.map((entry) => (
            <Row key={entry.invocationId} title={entry.toolId} detail={timeAgo(entry.createdAt)} state={entry.status} />
          ))}
          {audit.length === 0 && <Empty text="No tool invocations recorded" />}
        </Panel>
      </section>

      <section className="mt-6">
        <ProcessAllowlist />
      </section>
    </div>
  );
}

function FilterButton({ label, active, onClick }: { readonly label: string; readonly active: boolean; readonly onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`block w-full px-3 py-1.5 text-left font-label-mono text-[10px] uppercase tracking-wider ${
        active ? "bg-primary/10 text-primary" : "text-outline hover:text-on-surface"
      }`}
    >
      {label}
    </button>
  );
}

function Panel({ title, children }: { readonly title: string; readonly children: React.ReactNode }) {
  return <div className="border border-outline/20 p-5 card-ghost"><h3 className="font-label-mono text-[10px] uppercase tracking-widest text-primary mb-3">[{title}]</h3>{children}</div>;
}

function Row({ title, detail, state }: { readonly title: string; readonly detail: string; readonly state: string }) {
  return <div className="flex items-center justify-between gap-3 py-3 border-b border-outline-variant/10"><div className="min-w-0"><p className="text-sm text-on-surface truncate">{title}</p><p className="font-label-mono text-[10px] text-outline truncate">{detail}</p></div><span className="font-label-mono text-[9px] uppercase text-primary">[{state}]</span></div>;
}

function Empty({ text }: { readonly text: string }) {
  return <p className="py-8 text-center font-label-mono text-xs text-outline">{text}</p>;
}
