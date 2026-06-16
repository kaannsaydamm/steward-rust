import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Steward Agent OS",
  description: "Multi-channel Agent Operating System — Knowledge Engine, Workflow Orchestrator, Agent Monitor",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="h-full antialiased">
      <body className="h-full bg-background text-on-surface font-body selection:bg-primary/30">
        {/* Background Engraving — alchemical diagram watermark */}
        <div
          className="fixed inset-0 z-0 mix-blend-overlay opacity-[0.08] pointer-events-none"
          style={{
            backgroundImage: "url('https://lh3.googleusercontent.com/aida-public/AB6AXuDsaGsBYtuNJKAthHVEInF7R49o1GFhZlG0H13VxMkRHYpoF-t46koqBGV9RV0ewlxhmEVHFN4hHuK8iCIf4HPrEhxXopIKuGK3Wrujs9hF-fe9xNkAvE9gg045F1XGIOgmKpqM0oLoKrcAoLvcYfpzuDpTztIxthmOBgsJKhcuwPVm1ER3ahDp2Wx2KXXn2698s3hSjJFsnJPJe080fflXHSdXzVx_oARuugZc2i5Ag3YnmPB2nUZfEWtCm-IS-fynuptIUGFB0c4')",
            backgroundSize: "cover",
            backgroundPosition: "center",
          }}
        />
        {/* Noise Overlay — film grain texture */}
        <div className="noise-overlay" />
        {/* Main Content */}
        {children}
      </body>
    </html>
  );
}
