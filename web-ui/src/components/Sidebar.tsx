"use client";

export type TabId = "chat" | "dashboard" | "providers" | "knowledge" | "workflows" | "agents" | "capabilities";

interface SidebarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  daemonStatus: string;
}

const NAV_ITEMS: { id: TabId; label: string; icon: string }[] = [
  { id: "chat", label: "Chat", icon: ">_" },
  { id: "dashboard", label: "Dashboard", icon: "◈" },
  { id: "providers", label: "Providers", icon: "M" },
  { id: "knowledge", label: "Knowledge", icon: "⬡" },
  { id: "workflows", label: "Workflows", icon: "▶" },
  { id: "agents", label: "Agents", icon: "●" },
  { id: "capabilities", label: "Capabilities", icon: "◇" },
];

export default function Sidebar({ activeTab, onTabChange, daemonStatus }: SidebarProps) {
  const isConnected = daemonStatus.includes("Connected");

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
          Steward
        </span>
      </div>

      {/* ── Navigation ── */}
      <div className="flex flex-col space-y-0.5 px-2 md:px-3 mb-6">
        {NAV_ITEMS.map((item) => {
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onTabChange(item.id)}
              aria-label={item.label}
              title={item.label}
              className={`flex items-center justify-center md:justify-start gap-3 px-2 md:pl-4 py-2 text-sm transition-all duration-150 ${
                isActive
                  ? "text-primary border-l-2 border-primary bg-primary/5"
                  : "text-on-surface-variant/50 hover:text-on-surface hover:bg-primary/5 hover:text-primary border-l-2 border-transparent"
              }`}
            >
              <span className="text-sm w-5 text-center shrink-0">{item.icon}</span>
              <span className="hidden md:inline font-body-md text-sm">{item.label}</span>
            </button>
          );
        })}
      </div>

      {/* ── Status Section ── */}
      <div className="hidden md:block px-5 mb-4">
        <h3 className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50 mb-3 flex items-center gap-2">
          <span className="inline-block w-1 h-1 rounded-full bg-outline-variant/50" />
          SYSTEM
        </h3>
        <div className="flex items-center gap-2 px-1">
          <span
            className={`inline-block w-2 h-2 rounded-full ${
              isConnected ? "bg-primary" : "bg-error"
            }`}
          />
          <span className={`font-label-mono text-[10px] uppercase tracking-wider ${
            isConnected ? "text-on-surface-variant/60" : "text-error"
          }`}>
            {isConnected ? "[DAEMON_ACTIVE]" : "[DAEMON_LOST]"}
          </span>
        </div>
      </div>

      {/* ── Spacer / bottom section ── */}
      <div className="mt-auto px-2 md:px-5 py-4 border-t border-outline-variant/20">
        <div className="flex items-center">
          <div className="hidden md:block ml-auto">
            <span className="font-label-mono text-[9px] uppercase tracking-wider text-on-surface-variant/30">
              v0.1.0
            </span>
          </div>
        </div>
      </div>
    </nav>
  );
}
