"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient, timeAgo, formatInterval } from "@/lib/types";
import type { CronJobInfo, ToolInfo } from "@/lib/types";
import { StatusPill, type StatusTone } from "./StatusPill";

const INTERVAL_PRESETS = [
  { label: "5 minutes", seconds: 5 * 60 },
  { label: "15 minutes", seconds: 15 * 60 },
  { label: "1 hour", seconds: 60 * 60 },
  { label: "6 hours", seconds: 6 * 60 * 60 },
  { label: "1 day", seconds: 24 * 60 * 60 },
];

function statusTone(status: string): StatusTone {
  const normalized = status.toLowerCase();
  if (normalized.includes("succeeded")) return "success";
  if (normalized.includes("failed") || normalized.includes("error")) return "error";
  if (normalized.includes("denied") || normalized.includes("pending")) return "warning";
  return "neutral";
}

export default function CronTab() {
  const [jobs, setJobs] = useState<CronJobInfo[]>([]);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [name, setName] = useState("");
  const [toolId, setToolId] = useState("");
  const [intervalSeconds, setIntervalSeconds] = useState(INTERVAL_PRESETS[0].seconds);
  const [inputJson, setInputJson] = useState("{}");

  const load = useCallback(async () => {
    try {
      const [jobsResponse, toolsResponse] = await Promise.all([
        stewardClient.listCronJobs({}),
        stewardClient.listTools({ includeDisabled: false }),
      ]);
      setJobs(jobsResponse.jobs);
      setTools(toolsResponse.tools);
      if (!toolId && toolsResponse.tools.length > 0) {
        setToolId(toolsResponse.tools[0].toolId);
      }
      setError("");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void load(), 0);
    const interval = window.setInterval(() => void load(), 15_000);
    return () => {
      window.clearTimeout(timer);
      window.clearInterval(interval);
    };
  }, [load]);

  const create = async (event: FormEvent) => {
    event.preventDefault();
    setBusy("create");
    try {
      await stewardClient.createCronJob({
        name: name.trim(),
        toolId,
        inputJson: inputJson.trim() || "{}",
        intervalSeconds,
      });
      setName("");
      setInputJson("{}");
      await load();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    } finally {
      setBusy("");
    }
  };

  const toggle = async (job: CronJobInfo) => {
    setBusy(job.jobId);
    try {
      await stewardClient.setCronJobEnabled({ jobId: job.jobId, enabled: !job.enabled });
      await load();
    } finally {
      setBusy("");
    }
  };

  const runNow = async (job: CronJobInfo) => {
    setBusy(job.jobId);
    try {
      await stewardClient.runCronJobNow({ jobId: job.jobId });
      await load();
    } finally {
      setBusy("");
    }
  };

  const remove = async (job: CronJobInfo) => {
    setBusy(job.jobId);
    try {
      await stewardClient.deleteCronJob({ jobId: job.jobId });
      await load();
    } finally {
      setBusy("");
    }
  };

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-8">
      <div className="mx-auto max-w-6xl">
        <header className="mb-8">
          <p className="font-mono text-[10px] uppercase tracking-[0.25em] text-primary">Automation</p>
          <h2 className="mt-2 font-serif text-3xl">Cron Jobs</h2>
          <p className="mt-2 max-w-2xl text-sm text-outline">
            Schedule a governed tool to run on a recurring interval. Creating a job here is the
            approval for that tool + arguments — it will run unattended, so only enabled tools you
            trust with these exact arguments belong on a schedule.
          </p>
        </header>

        {error && <div className="mb-5 border border-error/40 p-3 text-sm text-error">{error}</div>}

        <form onSubmit={create} className="card-ghost mb-8 grid gap-4 p-5 md:grid-cols-2">
          <label className="block text-xs text-outline">Name
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Nightly memory sweep"
              className="input-ledger mt-1 w-full"
            />
          </label>
          <label className="block text-xs text-outline">Tool
            <select
              value={toolId}
              onChange={(event) => setToolId(event.target.value)}
              className="mt-1 w-full border border-outline-variant/50 bg-surface-container p-3 text-sm"
            >
              {tools.map((tool) => (
                <option key={tool.toolId} value={tool.toolId}>{tool.name}</option>
              ))}
            </select>
          </label>
          <label className="block text-xs text-outline">Interval
            <select
              value={intervalSeconds}
              onChange={(event) => setIntervalSeconds(Number(event.target.value))}
              className="mt-1 w-full border border-outline-variant/50 bg-surface-container p-3 text-sm"
            >
              {INTERVAL_PRESETS.map((preset) => (
                <option key={preset.seconds} value={preset.seconds}>{preset.label}</option>
              ))}
            </select>
          </label>
          <label className="block text-xs text-outline">Arguments (JSON)
            <input
              value={inputJson}
              onChange={(event) => setInputJson(event.target.value)}
              placeholder="{}"
              className="input-ledger mt-1 w-full font-mono"
            />
          </label>
          <button
            className="btn-ghost md:col-span-2"
            disabled={busy === "create" || !name.trim() || !toolId}
          >
            Schedule job
          </button>
        </form>

        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
          {jobs.map((job) => (
            <div key={job.jobId} className="card-ghost flex flex-col gap-3 p-5">
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                  <p className="truncate text-sm text-on-surface">{job.name}</p>
                  <p className="truncate font-mono text-[10px] text-outline">{job.toolId}</p>
                </div>
                <StatusPill tone={job.enabled ? "success" : "neutral"}>
                  {job.enabled ? "Enabled" : "Disabled"}
                </StatusPill>
              </div>
              <div className="flex items-center justify-between font-mono text-[11px] text-outline">
                <span>every {formatInterval(job.intervalSeconds)}</span>
                <span>{job.lastRunAt ? timeAgo(job.lastRunAt) : "never run"}</span>
              </div>
              {job.lastStatus && (
                <StatusPill tone={statusTone(job.lastStatus)}>{job.lastStatus}</StatusPill>
              )}
              <div className="mt-auto flex flex-wrap gap-2 pt-2">
                <button
                  type="button"
                  disabled={busy === job.jobId}
                  onClick={() => void runNow(job)}
                  className="border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-primary disabled:opacity-40"
                >
                  Run now
                </button>
                <button
                  type="button"
                  disabled={busy === job.jobId}
                  onClick={() => void toggle(job)}
                  className="border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-primary disabled:opacity-40"
                >
                  {job.enabled ? "Disable" : "Enable"}
                </button>
                <button
                  type="button"
                  disabled={busy === job.jobId}
                  onClick={() => void remove(job)}
                  className="border border-error/30 px-3 py-1 font-label-mono text-[9px] uppercase text-error disabled:opacity-40"
                >
                  Remove
                </button>
              </div>
            </div>
          ))}
          {jobs.length === 0 && (
            <p className="col-span-full py-8 text-center font-label-mono text-xs text-outline">
              No cron jobs scheduled yet
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
