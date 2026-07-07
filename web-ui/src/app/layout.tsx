import type { Metadata } from "next";
import Script from "next/script";
import "@xterm/xterm/css/xterm.css";
import "./globals.css";
import { LanguageProvider } from "@/lib/i18n/context";

export const metadata: Metadata = {
  title: "Steward Agent OS",
  description: "Local Steward workflows, tools, memory, and agent monitoring",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="h-full antialiased">
      <head>
        <Script src="/steward-config.js" strategy="beforeInteractive" />
      </head>
      <body className="h-full bg-background text-on-surface font-body selection:bg-primary/30">
        <div
          className="fixed inset-0 z-0 opacity-30 pointer-events-none"
          style={{
            backgroundImage:
              "radial-gradient(circle at 85% 10%, rgba(96, 196, 206, 0.12), transparent 30%), radial-gradient(circle at 15% 85%, rgba(242, 202, 80, 0.08), transparent 32%)",
          }}
        />
        <div className="noise-overlay" />
        <LanguageProvider>{children}</LanguageProvider>
      </body>
    </html>
  );
}
