"use client";

import { useState, useEffect, useCallback } from "react";
import { stewardClient } from "@/lib/grpc";
import Sidebar from "@/components/Sidebar";
import DashboardTab from "@/components/DashboardTab";
import KGViewer from "@/components/KGViewer";
import WorkflowsTab from "@/components/WorkflowsTab";
import AgentsTab from "@/components/AgentsTab";
import type { TabId } from "@/components/Sidebar";

export default function Home() {
  const [status, setStatus] = useState("Connecting...");
  const [activeTab, setActiveTab] = useState<TabId>("dashboard");

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const response = await stewardClient.ping({});
        setStatus(response.status || "Connected");
      } catch (err: any) {
        setStatus(`Disconnected`);
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const renderContent = () => {
    switch (activeTab) {
      case "dashboard":
        return <DashboardTab />;
      case "knowledge":
        return <KGViewer />;
      case "workflows":
        return <WorkflowsTab />;
      case "agents":
        return <AgentsTab />;
      default:
        return <DashboardTab />;
    }
  };

  const isConnected = status.includes("Connected");

  return (
    <div className="relative z-10 flex h-screen bg-background overflow-hidden">
      <Sidebar activeTab={activeTab} onTabChange={setActiveTab} daemonStatus={status} />
      <div className="flex-1 flex flex-col overflow-hidden min-w-0">
        {/* ── Header Bar ── */}
        <header className="h-12 shrink-0 flex items-center justify-between px-6 border-b border-outline-variant/20 bg-background/90 backdrop-blur-md z-20">
          <div className="flex items-center gap-6">
            <span className="font-label-mono text-label-mono uppercase tracking-wider text-on-surface-variant/50">
              Steward Agent OS
            </span>
            <span className={`font-label-mono text-[10px] uppercase tracking-wider ${isConnected ? "text-primary" : "text-error"}`}>
              [{isConnected ? "DAEMON_ONLINE" : "DAEMON_OFFLINE"}]
            </span>
          </div>
          <div className="flex items-center gap-3 text-on-surface-variant/40">
            <span className="font-label-mono text-[10px] uppercase tracking-widest">
              v0.2.0
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
      </div>
    </div>
  );
}
