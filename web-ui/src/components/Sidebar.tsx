"use client";
import { useEffect, useState } from "react";
import {
  Bot,
  Clock,
  FolderArchive,
  History,
  LayoutDashboard,
  MessageSquareText,
  Settings,
  ShieldCheck,
  Share2,
  Cpu,
  TerminalSquare,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { useTranslation } from "@/lib/i18n/context";
import { stewardClient } from "@/lib/grpc";
import LanguageSwitcher from "./LanguageSwitcher";

type NavLabelKey =
  | "nav.chat"
  | "nav.dashboard"
  | "nav.providers"
  | "nav.knowledge"
  | "nav.workflows"
  | "nav.agents"
  | "nav.capabilities"
  | "nav.cron"
  | "nav.artifacts"
  | "nav.sessions"
  | "nav.settings"
  | "nav.commandCenter";


export type TabId =
  | "chat"
  | "dashboard"
  | "providers"
  | "knowledge"
  | "workflows"
  | "agents"
  | "capabilities"
  | "cron"
  | "artifacts"
  // Hermes shell navigation additions (routes.ts parity).
  | "sessions"
  | "settings"
  | "command-center";

interface SidebarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  daemonStatus: string;
}

const NAV_ITEMS: { id: TabId; labelKey: NavLabelKey; icon: LucideIcon }[] = [
  { id: "chat", labelKey: "nav.chat", icon: MessageSquareText },
  { id: "dashboard", labelKey: "nav.dashboard", icon: LayoutDashboard },
  { id: "sessions", labelKey: "nav.sessions", icon: History },
  { id: "providers", labelKey: "nav.providers", icon: Cpu },
  { id: "knowledge", labelKey: "nav.knowledge", icon: Share2 },
  { id: "workflows", labelKey: "nav.workflows", icon: Workflow },
  { id: "agents", labelKey: "nav.agents", icon: Bot },
  { id: "capabilities", labelKey: "nav.capabilities", icon: ShieldCheck },
  { id: "cron", labelKey: "nav.cron", icon: Clock },
  { id: "artifacts", labelKey: "nav.artifacts", icon: FolderArchive },
  { id: "settings", labelKey: "nav.settings", icon: Settings },
  { id: "command-center", labelKey: "nav.commandCenter", icon: TerminalSquare },
];

export default function Sidebar({ activeTab, onTabChange, daemonStatus }: SidebarProps) {
  const { t } = useTranslation();
  const isConnected = daemonStatus.includes("Connected");
  const [counts, setCounts] = useState({ agents: 0, workflows: 0 });

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const [agentsRes, workflowsRes] = await Promise.all([
          stewardClient.listAgents({}),
          stewardClient.listWorkflows({ phaseFilter: 0, limit: 100 }),
        ]);
        if (!cancelled) {
          setCounts({ agents: agentsRes.agents.length, workflows: workflowsRes.workflows.length });
        }
      } catch {
        // daemon unreachable; leave last-known counts in place
      }
    };
    const timer = window.setTimeout(() => void load(), 0);
    const interval = window.setInterval(() => void load(), 10_000);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      window.clearInterval(interval);
    };
  }, []);

  return (
    <nav className="w-16 md:w-64 shrink-0 h-full flex flex-col border-r border-outline-variant/30 bg-background/80 backdrop-blur-xl z-30">
      {/* ── Workspace Header ── */}
      <div className="flex items-center px-3 md:px-5 pt-5 pb-6">
        {/* The daemon serves a static export, so this local asset must not use Next's image API. */}
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img
          src="/steward-logo.png"
          alt="Steward"
          className="w-8 h-8 object-contain mr-2 md:mr-3"
        />
        <span className="hidden md:inline font-label-mono text-[11px] uppercase tracking-widest text-on-surface-variant/50">
          {t("sidebar.brand")}
        </span>
      </div>

      {/* ── Navigation ── */}
      <div className="flex flex-col space-y-0.5 px-2 md:px-3 mb-6">
        {NAV_ITEMS.map((item) => {
          const isActive = activeTab === item.id;
          const Icon = item.icon;
          const label = t(item.labelKey);
          return (
            <button
              key={item.id}
              onClick={() => onTabChange(item.id)}
              aria-label={label}
              title={label}
              aria-current={isActive ? "page" : undefined}
              className={`group relative flex items-center justify-center md:justify-start gap-3 px-2 md:pl-4 py-2 text-sm transition-colors duration-150 cursor-pointer ${
                isActive
                  ? "nav-active text-primary border-l-2 border-primary"
                  : "text-on-surface-variant/50 hover:text-primary hover:bg-primary/[0.04] border-l-2 border-transparent"
              }`}
            >
              <Icon
                className={`w-4 h-4 shrink-0 transition-transform duration-150 ${isActive ? "" : "group-hover:scale-110"}`}
                strokeWidth={isActive ? 2 : 1.75}
              />
              <span className="hidden md:inline font-body-md text-sm">{label}</span>
              {isActive && (
                <span className="absolute right-2 hidden md:block h-1 w-1 rounded-full bg-primary shadow-[0_0_6px_rgba(242,202,80,0.8)]" />
              )}
            </button>
          );
        })}
      </div>

      {/* ── Persistent system panel (visible on every tab, like a status footer) ── */}
      <div className="hidden md:block mt-auto px-5 py-4 border-t border-outline-variant/20">
        <h3 className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50 mb-3">
          {t("sidebar.system")}
        </h3>
        <div className="space-y-1.5 mb-3">
          <div className="flex items-center gap-2">
            <span
              className={`inline-block w-2 h-2 rounded-full ${isConnected ? "bg-primary dot-live" : "bg-error"}`}
            />
            <span
              className={`font-label-mono text-[10px] uppercase tracking-wider ${
                isConnected ? "text-on-surface-variant/60" : "text-error"
              }`}
            >
              {isConnected ? t("sidebar.daemonActive") : t("sidebar.daemonLost")}
            </span>
          </div>
          <div className="flex items-center justify-between text-[11px] text-on-surface-variant/50">
            <span>{t("sidebar.agents")}</span>
            <span className="stat-value font-mono text-on-surface-variant/80">{counts.agents}</span>
          </div>
          <div className="flex items-center justify-between text-[11px] text-on-surface-variant/50">
            <span>{t("sidebar.workflows")}</span>
            <span className="stat-value font-mono text-on-surface-variant/80">{counts.workflows}</span>
          </div>
        </div>
        <div className="mb-3">
          <LanguageSwitcher />
        </div>
        <span className="font-label-mono text-[9px] uppercase tracking-wider text-on-surface-variant/30">
          v0.1.0
        </span>
      </div>
    </nav>
  );
}
