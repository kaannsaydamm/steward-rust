"use client";

import { useEffect, useState } from "react";
import {
  Bot,
  Clock,
  FolderArchive,
  LayoutDashboard,
  MessageSquareText,
  ShieldCheck,
  Share2,
  Cpu,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { stewardClient } from "@/lib/grpc";
import { useTranslation } from "@/lib/i18n/context";
import LanguageSwitcher from "./LanguageSwitcher";

export type TabId =
  | "chat"
  | "dashboard"
  | "providers"
  | "knowledge"
  | "workflows"
  | "agents"
  | "capabilities"
  | "cron"
  | "artifacts";

interface SidebarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  daemonStatus: string;
}

const NAV_ITEMS: { id: TabId; labelKey: "nav.chat" | "nav.dashboard" | "nav.providers" | "nav.knowledge" | "nav.workflows" | "nav.agents" | "nav.capabilities" | "nav.cron" | "nav.artifacts"; icon: LucideIcon }[] = [
  { id: "chat", labelKey: "nav.chat", icon: MessageSquareText },
  { id: "dashboard", labelKey: "nav.dashboard", icon: LayoutDashboard },
  { id: "providers", labelKey: "nav.providers", icon: Cpu },
  { id: "knowledge", labelKey: "nav.knowledge", icon: Share2 },
  { id: "workflows", labelKey: "nav.workflows", icon: Workflow },
  { id: "agents", labelKey: "nav.agents", icon: Bot },
  { id: "capabilities", labelKey: "nav.capabilities", icon: ShieldCheck },
  { id: "cron", labelKey: "nav.cron", icon: Clock },
  { id: "artifacts", labelKey: "nav.artifacts", icon: FolderArchive },
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
              className={`flex items-center justify-center md:justify-start gap-3 px-2 md:pl-4 py-2 text-sm transition-all duration-150 ${
                isActive
                  ? "text-primary border-l-2 border-primary bg-primary/5"
                  : "text-on-surface-variant/50 hover:text-on-surface hover:bg-primary/5 hover:text-primary border-l-2 border-transparent"
              }`}
            >
              <Icon className="w-4 h-4 shrink-0" strokeWidth={1.75} />
              <span className="hidden md:inline font-body-md text-sm">{label}</span>
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
              className={`inline-block w-2 h-2 rounded-full ${isConnected ? "bg-primary" : "bg-error"}`}
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
            <span className="font-mono text-on-surface-variant/80">{counts.agents}</span>
          </div>
          <div className="flex items-center justify-between text-[11px] text-on-surface-variant/50">
            <span>{t("sidebar.workflows")}</span>
            <span className="font-mono text-on-surface-variant/80">{counts.workflows}</span>
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
