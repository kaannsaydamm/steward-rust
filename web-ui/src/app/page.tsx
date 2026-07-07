"use client";

import { useState, useEffect, useRef } from "react";
import { ChevronUp, ChevronDown, TerminalSquare } from "lucide-react";
import { stewardClient } from "@/lib/grpc";
import Sidebar from "@/components/Sidebar";
import DashboardTab from "@/components/DashboardTab";
import KGViewer from "@/components/KGViewer";
import WorkflowsTab from "@/components/WorkflowsTab";
import AgentsTab from "@/components/AgentsTab";
import CapabilitiesTab from "@/components/CapabilitiesTab";
import ChatTab from "@/components/ChatTab";
import ProvidersTab from "@/components/ProvidersTab";
import CronTab from "@/components/CronTab";
import Terminal, { type TerminalHandle } from "@/components/Terminal";
import type { TabId } from "@/components/Sidebar";

const TERMINAL_TRANSITION_MS = 150;

export default function Home() {
  const [status, setStatus] = useState("Connecting...");
  const [activeTab, setActiveTab] = useState<TabId>("chat");
  const [workflowCreatorOpen, setWorkflowCreatorOpen] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const terminalRef = useRef<TerminalHandle>(null);

  useEffect(() => {
    if (!terminalOpen) return;
    const timer = window.setTimeout(() => {
      terminalRef.current?.refit();
    }, TERMINAL_TRANSITION_MS + 20);
    return () => window.clearTimeout(timer);
  }, [terminalOpen]);

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const response = await stewardClient.ping({});
        setStatus(`Connected: ${response.status || "OK"}`);
      } catch {
        setStatus(`Disconnected`);
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const renderContent = () => {
    switch (activeTab) {
      case "chat":
        return <ChatTab onNavigateToProviders={() => setActiveTab("providers")} />;
      case "dashboard":
        return (
          <DashboardTab
            isConnected={isConnected}
            onNavigate={(tab) => {
              setWorkflowCreatorOpen(tab === "workflows");
              setActiveTab(tab);
            }}
          />
        );
      case "knowledge":
        return <KGViewer />;
      case "workflows":
        return (
          <WorkflowsTab
            initiallyOpenCreator={workflowCreatorOpen}
          />
        );
      case "agents":
        return <AgentsTab />;
      case "capabilities":
        return <CapabilitiesTab />;
      case "providers":
        return <ProvidersTab />;
      case "cron":
        return <CronTab />;
      default:
        return <DashboardTab isConnected={isConnected} onNavigate={setActiveTab} />;
    }
  };

  const isConnected = status.includes("Connected");

  return (
    <div className="relative z-10 flex h-screen bg-background overflow-hidden">
      <Sidebar
        activeTab={activeTab}
        onTabChange={(tab) => {
          setWorkflowCreatorOpen(false);
          setActiveTab(tab);
        }}
        daemonStatus={status}
      />
      <div className="flex-1 flex flex-col overflow-hidden min-w-0">
        {/* ── Header Bar ── */}
        <header className="h-12 shrink-0 flex items-center justify-between px-3 md:px-6 border-b border-outline-variant/20 bg-background/90 backdrop-blur-md z-20">
          <div className="flex items-center gap-6">
            <span className="hidden sm:inline font-label-mono text-label-mono uppercase tracking-wider text-on-surface-variant/50">
              Steward Agent OS
            </span>
            <span className={`font-label-mono text-[10px] uppercase tracking-wider ${isConnected ? "text-primary" : "text-error"}`}>
              [{isConnected ? "DAEMON_ONLINE" : "DAEMON_OFFLINE"}]
            </span>
          </div>
          <div className="flex items-center gap-3 text-on-surface-variant/40">
            <span className="font-label-mono text-[10px] uppercase tracking-widest">
              v0.1.0
            </span>
            <span className="w-px h-3 bg-outline-variant/30" />
            <span className="font-label-mono text-[10px] uppercase tracking-widest">
              {activeTab}
            </span>
          </div>
        </header>
        {/* ── Main Content ── */}
        <main className="flex-1 flex flex-col overflow-hidden min-w-0">
          {renderContent()}
        </main>

        {/* ── Global terminal panel (persists across tabs, toggled like a VS Code panel) ── */}
        <div
          className={`shrink-0 border-t border-outline-variant/20 transition-[height] duration-150 ${
            terminalOpen ? "h-64 sm:h-72 lg:h-80" : "h-0"
          } overflow-hidden`}
        >
          <Terminal ref={terminalRef} />
        </div>
        <button
          type="button"
          onClick={() => setTerminalOpen((open) => !open)}
          className="flex h-8 shrink-0 items-center gap-2 border-t border-outline-variant/20 bg-surface-container-lowest px-3 md:px-6 text-on-surface-variant/60 hover:text-primary"
        >
          <TerminalSquare className="h-3.5 w-3.5" strokeWidth={1.75} />
          <span className="font-label-mono text-[10px] uppercase tracking-widest">Terminal</span>
          {terminalOpen ? (
            <ChevronDown className="h-3 w-3" strokeWidth={2} />
          ) : (
            <ChevronUp className="h-3 w-3" strokeWidth={2} />
          )}
        </button>
      </div>
    </div>
  );
}
