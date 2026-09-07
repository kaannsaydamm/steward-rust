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
import ArtifactsTab from "@/components/ArtifactsTab";
import SessionsTab from "@/components/SessionsTab";
import SettingsTab from "@/components/SettingsTab";
import CommandCenterTab from "@/components/CommandCenterTab";
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

  // Boot-failure overlay state (Hermes boot-failure-overlay semantics):
  const isConnected = status.includes("Connected");
  // three consecutive failed pings show the failure overlay with retry.
  const [bootFailures, setBootFailures] = useState(0);
  const [bootFailureVisible, setBootFailureVisible] = useState(false);
  // First-run onboarding: no provider profile configured → setup dialog.
  const [profilesLoaded, setProfilesLoaded] = useState(false);
  const [onboardingVisible, setOnboardingVisible] = useState(false);

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const response = await stewardClient.ping({});
        setStatus(`Connected: ${response.status || "OK"}`);
        setBootFailures(0);
        setBootFailureVisible(false);
      } catch {
        setStatus(`Disconnected`);
        setBootFailures((failures) => {
          const next = failures + 1;
          if (next >= 3) setBootFailureVisible(true);
          return next;
        });
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, [isConnected, profilesLoaded]);

  useEffect(() => {
    if (!isConnected || profilesLoaded) return;
    stewardClient
      .listProviderProfiles({})
      .then((response) => {
        setProfilesLoaded(true);
        if (response.profiles.length === 0) setOnboardingVisible(true);
      })
      .catch(() => undefined);
  }, [isConnected, profilesLoaded]);

  // Alt+1..9 tab jump (Hermes keymap: quick tab switching).
  const NAV_ORDER: TabId[] = [
    "chat",
    "dashboard",
    "sessions",
    "providers",
    "knowledge",
    "workflows",
    "agents",
    "capabilities",
    "cron",
  ];
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.altKey) return;
      const digit = Number(event.key);
      if (!Number.isInteger(digit) || digit < 1 || digit > NAV_ORDER.length) return;
      event.preventDefault();
      setActiveTab(NAV_ORDER[digit - 1]);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
  const renderContent = () => {
    switch (activeTab) {
      case "chat":
        return <ChatTab onNavigate={setActiveTab} daemonOnline={isConnected} />;
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
      case "artifacts":
        return <ArtifactsTab />;
      case "sessions":
        return <SessionsTab onOpenChat={() => setActiveTab("chat")} />;
      case "settings":
        return <SettingsTab />;
      case "command-center":
        return <CommandCenterTab />;
      default:
        return <DashboardTab isConnected={isConnected} onNavigate={setActiveTab} />;
    }
  };


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
      {/* ── Boot-failure overlay (Hermes semantics: 3 failed pings + retry) ── */}
      {bootFailureVisible && (
        <div role="alertdialog" aria-label="daemon boot failure" className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70">
          <div className="composer-surface max-w-md p-8 text-center">
            <p className="mb-2 font-label-mono text-[10px] uppercase tracking-widest text-error">daemon unreachable</p>
            <h3 className="mb-3 font-serif text-xl text-on-surface">Steward daemon did not start</h3>
            <p className="mb-6 text-sm text-on-surface-variant/60">
              Start the daemon, then retry. The shell will keep polling in the background.
            </p>
            <button
              type="button"
              onClick={() => {
                setBootFailures(0);
                setBootFailureVisible(false);
              }}
              className="btn-ghost border border-primary/40 px-4 py-2 font-mono text-[11px] uppercase text-primary"
            >
              Retry
            </button>
          </div>
        </div>
      )}
      {/* ── First-run onboarding: provider profile setup ── */}
      {onboardingVisible && !bootFailureVisible && (
        <div role="dialog" aria-label="first run setup" className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70">
          <div className="composer-surface max-w-md p-8 text-center">
            <h3 className="mb-3 font-serif text-xl text-on-surface">Welcome to Steward</h3>
            <p className="mb-6 text-sm text-on-surface-variant/60">
              No provider profile is configured yet. Add one to start chatting — your API key goes into the OS vault, never into plain files.
            </p>
            <div className="flex items-center justify-center gap-2">
              <button
                type="button"
                onClick={() => {
                  setOnboardingVisible(false);
                  setActiveTab("providers");
                }}
                className="btn-ghost border border-primary/40 px-4 py-2 font-mono text-[11px] uppercase text-primary"
              >
                Set up providers
              </button>
              <button
                type="button"
                onClick={() => setOnboardingVisible(false)}
                className="btn-ghost border border-outline-variant/40 px-4 py-2 font-mono text-[11px] uppercase text-on-surface-variant/70"
              >
                Later
              </button>
            </div>
          </div>
        </div>
      )}
      <div className="flex-1 flex flex-col overflow-hidden min-w-0">
        {/* ── Header Bar ── */}
        <header className="h-12 shrink-0 flex items-center justify-between px-3 md:px-6 border-b border-outline-variant/20 bg-background/90 backdrop-blur-md z-20">
          <div className="flex items-center gap-6">
            <span className="hidden sm:inline font-label-mono text-label-mono uppercase tracking-wider text-on-surface-variant/50">
              Steward Agent OS
            </span>
            <span className="flex items-center gap-2">
              <span className={`inline-block w-1.5 h-1.5 rounded-full ${isConnected ? "bg-primary dot-live" : "bg-error"}`} />
              <span className={`font-label-mono text-[10px] uppercase tracking-wider ${isConnected ? "text-primary" : "text-error"}`}>
                {isConnected ? "DAEMON_ONLINE" : "DAEMON_OFFLINE"}
              </span>
            </span>
          </div>
          <div className="flex items-center gap-3 text-on-surface-variant/40">
            <span className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/40">
              v0.1.0
            </span>
            <span className="w-px h-3 bg-outline-variant/30" />
            <span className="border border-outline/25 px-2 py-0.5 font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/70">
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
