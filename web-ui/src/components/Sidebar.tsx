"use client";

export type TabId = "dashboard" | "knowledge" | "workflows" | "agents";

interface SidebarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  daemonStatus: string;
}

const NAV_ITEMS: { id: TabId; label: string; icon: string }[] = [
  { id: "dashboard", label: "Dashboard", icon: "◈" },
  { id: "knowledge", label: "Knowledge", icon: "⬡" },
  { id: "workflows", label: "Workflows", icon: "▶" },
  { id: "agents", label: "Agents", icon: "●" },
];

export default function Sidebar({ activeTab, onTabChange, daemonStatus }: SidebarProps) {
  const isConnected = daemonStatus.includes("Connected");

  return (
    <nav className="w-64 shrink-0 h-full flex flex-col border-r border-outline-variant/30 bg-background/80 backdrop-blur-xl z-30">
      {/* ── Workspace Header ── */}
      <div className="flex items-center px-5 pt-5 pb-6">
        <div className="w-7 h-7 flex items-center justify-center border border-outline-variant/30 text-primary-container text-sm font-bold font-mono mr-3">
          S
        </div>
        <span className="font-label-mono text-[11px] uppercase tracking-widest text-on-surface-variant/50">
          Steward
        </span>
      </div>

      {/* ── Navigation ── */}
      <div className="flex flex-col space-y-0.5 px-3 mb-6">
        {NAV_ITEMS.map((item) => {
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onTabChange(item.id)}
              className={`flex items-center gap-3 pl-4 py-2 text-sm transition-all duration-150 ${
                isActive
                  ? "text-primary border-l-2 border-primary bg-primary/5"
                  : "text-on-surface-variant/50 hover:text-on-surface hover:bg-primary/5 hover:text-primary border-l-2 border-transparent"
              }`}
            >
              <span className="text-sm w-5 text-center shrink-0">{item.icon}</span>
              <span className="font-body-md text-sm">{item.label}</span>
            </button>
          );
        })}
      </div>

      {/* ── Status Section ── */}
      <div className="px-5 mb-4">
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
      <div className="mt-auto px-5 py-4 border-t border-outline-variant/20">
        <div className="flex items-center gap-1">
          <div className="w-6 h-6 flex items-center justify-center border border-outline-variant/30 cursor-pointer hover:border-primary transition-colors">
            <span className="font-label-mono text-[10px] text-on-surface-variant/50">◈</span>
          </div>
          <div className="w-6 h-6 flex items-center justify-center border border-outline-variant/30 cursor-pointer hover:border-primary transition-colors">
            <span className="font-label-mono text-[10px] text-on-surface-variant/50">⚙</span>
          </div>
          <div className="ml-auto">
            <span className="font-label-mono text-[9px] uppercase tracking-wider text-on-surface-variant/30">
              v0.2.0
            </span>
          </div>
        </div>
      </div>
    </nav>
  );
}
