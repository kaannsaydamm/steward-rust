const TONES = {
  success: "border-primary/40 text-primary bg-primary/10",
  warning: "border-sigil-blue/40 text-sigil-blue bg-sigil-blue/10",
  error: "border-error/40 text-error bg-error/10",
  neutral: "border-outline/30 text-outline bg-transparent",
} as const;

export type StatusTone = keyof typeof TONES;

export function StatusPill({
  children,
  tone = "neutral",
}: {
  readonly children: React.ReactNode;
  readonly tone?: StatusTone;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 border px-2 py-0.5 font-label-mono text-[9px] uppercase tracking-wider ${TONES[tone]}`}
    >
      <span className="w-1.5 h-1.5 rounded-full bg-current shrink-0" />
      {children}
    </span>
  );
}
