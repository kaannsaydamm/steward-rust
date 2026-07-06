"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient, timeAgo } from "@/lib/types";
import type {
  MaintenanceStatus,
  McpAdapterInfo,
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

export default function CapabilitiesTab() {
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [adapters, setAdapters] = useState<McpAdapterInfo[]>([]);
  const [audit, setAudit] = useState<ToolInvocationInfo[]>([]);
  const [maintenance, setMaintenance] = useState<MaintenanceStatus | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [adapterId, setAdapterId] = useState("");
  const [adapterName, setAdapterName] = useState("");
  const [adapterCommand, setAdapterCommand] = useState("");
  const [adapterArgs, setAdapterArgs] = useState("");

  const load = useCallback(async () => {
    try {
      const [toolResult, skillResult, adapterResult, auditResult, maintenanceResult] =
        await Promise.all([
          stewardClient.listTools({ includeDisabled: true }),
          stewardClient.listSkills({ includeDisabled: true }),
          stewardClient.listMcpAdapters({}),
          stewardClient.listToolInvocations({ limit: 20 }),
          stewardClient.getMaintenanceStatus({}),
        ]);
      setTools(toolResult.tools);
      setSkills(skillResult.skills);
      setAdapters(adapterResult.adapters);
      setAudit(auditResult.invocations);
      setMaintenance(maintenanceResult);
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

      <section className="grid grid-cols-1 xl:grid-cols-2 gap-6 mb-6">
        <Panel title={`Tools / ${tools.length}`}>
          {tools.map((tool) => (
            <div key={tool.toolId} className="flex items-center justify-between gap-3 py-3 border-b border-outline-variant/10">
              <div className="min-w-0">
                <p className="text-sm text-on-surface truncate">{tool.name}</p>
                <p className="font-label-mono text-[10px] text-outline truncate">{tool.toolId} · risk {tool.risk}</p>
              </div>
              <button
                type="button"
                disabled={busy !== ""}
                onClick={() => void toggleTool(tool)}
                className="border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-primary disabled:opacity-40"
              >
                [{tool.enabled ? "enabled" : "disabled"}]
              </button>
            </div>
          ))}
        </Panel>
        <Panel title={`Signed skills / ${skills.length}`}>
          {skills.map((skill) => (
            <Row key={skill.skillId} title={skill.name} detail={`${skill.skillId} · ${skill.toolIds.length} tools`} state={skill.signed ? "signed" : "unsigned"} />
          ))}
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

function Panel({ title, children }: { readonly title: string; readonly children: React.ReactNode }) {
  return <div className="border border-outline/20 p-5 card-ghost"><h3 className="font-label-mono text-[10px] uppercase tracking-widest text-primary mb-3">[{title}]</h3>{children}</div>;
}

function Row({ title, detail, state }: { readonly title: string; readonly detail: string; readonly state: string }) {
  return <div className="flex items-center justify-between gap-3 py-3 border-b border-outline-variant/10"><div className="min-w-0"><p className="text-sm text-on-surface truncate">{title}</p><p className="font-label-mono text-[10px] text-outline truncate">{detail}</p></div><span className="font-label-mono text-[9px] uppercase text-primary">[{state}]</span></div>;
}

function Empty({ text }: { readonly text: string }) {
  return <p className="py-8 text-center font-label-mono text-xs text-outline">{text}</p>;
}
