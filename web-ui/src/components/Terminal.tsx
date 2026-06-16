"use client";

import { useState } from "react";
import { stewardClient } from "@/lib/grpc";

interface TerminalProps {
  onTaskExecuted?: (msg: string) => void;
}

interface LogEntry {
  timestamp: string;
  message: string;
  type: "input" | "output" | "error" | "system";
}

export default function Terminal({ onTaskExecuted }: TerminalProps) {
  const [logs, setLogs] = useState<LogEntry[]>(() => [
    { timestamp: new Date().toLocaleTimeString(), message: "Steward OS Terminal ready. Type a command or task.", type: "system" as const },
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cmd = input.trim();
    if (!cmd || loading) return;

    setLogs((prev) => [
      ...prev,
      { timestamp: new Date().toLocaleTimeString(), message: `$ ${cmd}`, type: "input" as const },
    ]);
    setInput("");
    setLoading(true);

    try {
      const res = await stewardClient.executeTask({ task: cmd });
      setLogs((prev) => [
        ...prev,
        { timestamp: new Date().toLocaleTimeString(), message: res.status, type: "output" as const },
      ]);
      onTaskExecuted?.(res.status);
    } catch (err: any) {
      setLogs((prev) => [
        ...prev,
        { timestamp: new Date().toLocaleTimeString(), message: `Error: ${err.message}`, type: "error" as const },
      ]);
    } finally {
      setLoading(false);
    }
  };

  const clearLogs = () => {
    setLogs([
      { timestamp: new Date().toLocaleTimeString(), message: "Terminal cleared.", type: "system" as const },
    ]);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Terminal header */}
      <div className="flex items-center justify-between px-4 py-2 bg-surface-container-lowest border-b border-outline-variant/20">
        <div className="flex items-center gap-2">
          <span className="text-primary text-sm font-mono">❯</span>
          <span className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50">
            Terminal
          </span>
        </div>
        <button
          onClick={clearLogs}
          className="font-label-mono text-[10px] uppercase tracking-wider text-outline hover:text-error transition-colors"
        >
          Clear
        </button>
      </div>

      {/* Log area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-1.5 font-mono text-sm bg-deep-void/40">
        {logs.map((entry, i) => (
          <div key={i} className="leading-relaxed">
            <span className="text-outline/50 text-[10px] mr-2">[{entry.timestamp}]</span>
            {entry.type === "input" && (
              <span className="text-primary">{entry.message}</span>
            )}
            {entry.type === "output" && (
              <span className="text-on-surface/80">{entry.message}</span>
            )}
            {entry.type === "error" && (
              <span className="text-error">{entry.message}</span>
            )}
            {entry.type === "system" && (
              <span className="text-on-surface-variant/40 italic">{entry.message}</span>
            )}
          </div>
        ))}
        {loading && (
          <div className="text-primary/70 animate-pulse">
            <span className="text-outline/50 text-[10px] mr-2">[{new Date().toLocaleTimeString()}]</span>
            Processing...
          </div>
        )}
        <div className="cursor-blink text-primary text-sm inline-block" />
      </div>

      {/* Input */}
      <form onSubmit={handleSubmit} className="flex items-center gap-2 px-4 py-3 bg-surface-container-lowest border-t border-outline-variant/20">
        <span className="text-primary font-bold text-sm font-mono">$</span>
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Enter task or command..."
          disabled={loading}
          className="flex-1 bg-transparent border-none outline-none text-on-surface text-sm font-mono placeholder:text-outline/40"
        />
        <button
          type="submit"
          disabled={loading || !input.trim()}
          className="btn-ghost disabled:opacity-30"
        >
          {loading ? "..." : "Run"}
        </button>
      </form>
    </div>
  );
}
