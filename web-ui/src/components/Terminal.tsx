"use client";

import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";

function terminalSocketUrl(): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/terminal/ws`;
}

export interface TerminalHandle {
  refit: () => void;
}

const Terminal = forwardRef<TerminalHandle>(function Terminal(_props, ref) {
  const containerRef = useRef<HTMLDivElement>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const termRef = useRef<XTerm | null>(null);

  useImperativeHandle(ref, () => ({
    refit: () => {
      fitAddonRef.current?.fit();
      const term = termRef.current;
      if (term) term.refresh(0, term.rows - 1);
    },
  }));

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new XTerm({
      cursorBlink: true,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: 13,
      theme: {
        background: "#000000",
      },
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);
    fitAddon.fit();
    fitAddonRef.current = fitAddon;
    termRef.current = term;

    const socket = new WebSocket(terminalSocketUrl());
    socket.binaryType = "arraybuffer";

    const sendResize = () => {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ cols: term.cols, rows: term.rows }));
      }
    };

    socket.onopen = () => {
      fitAddon.fit();
      sendResize();
    };
    socket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        term.write(new Uint8Array(event.data));
      } else {
        term.write(String(event.data));
      }
    };
    socket.onclose = () => {
      term.write("\r\n\x1b[31mTerminal session ended.\x1b[0m\r\n");
    };
    socket.onerror = () => {
      term.write("\r\n\x1b[31mTerminal connection failed.\x1b[0m\r\n");
    };

    const dataDisposable = term.onData((data) => {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(new TextEncoder().encode(data));
      }
    });
    const resizeDisposable = term.onResize(sendResize);

    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      term.refresh(0, term.rows - 1);
    });
    resizeObserver.observe(container);

    document.fonts?.ready.then(() => {
      fitAddon.fit();
      term.refresh(0, term.rows - 1);
    });

    return () => {
      resizeObserver.disconnect();
      dataDisposable.dispose();
      resizeDisposable.dispose();
      socket.close();
      term.dispose();
      fitAddonRef.current = null;
      termRef.current = null;
    };
  }, []);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between px-4 py-2 bg-surface-container-lowest border-b border-outline-variant/20">
        <div className="flex items-center gap-2">
          <span className="text-primary text-sm font-mono">❯</span>
          <span className="font-label-mono text-[10px] uppercase tracking-widest text-on-surface-variant/50">
            Terminal
          </span>
        </div>
      </div>
      <div ref={containerRef} className="flex-1 overflow-hidden bg-deep-void/40 p-2" />
    </div>
  );
});

export default Terminal;
