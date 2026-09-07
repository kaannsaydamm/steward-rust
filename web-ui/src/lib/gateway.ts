// Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
// tui_gateway/ client architecture (MIT). Modified for Steward: the typed
// request/event client runs over Steward's gRPC-web daemon channel instead of
// Hermes's JSON-RPC WebSocket; resume uses GetChatSession(since_sequence)
// rather than `session.events.since`.

import { getStewardClient } from "./grpc";
import {
  ChatEventKind,
  type ChatMessageInfo,
  type ChatSessionSummary,
} from "./proto/steward";

/** Transport state surfaced to overlays (boot-failure, reconnect banners). */
export type GatewayState = "idle" | "connecting" | "online" | "reconnecting" | "failed";

export interface GatewayEvents {
  onState?(state: GatewayState): void;
  onError?(error: string): void;
}

const MAX_FAILURES_BEFORE_BOOT_FAILURE = 3;
const RECONNECT_DELAY_MS = 1_500;

/**
 * Gateway: single typed facade over the daemon RPC surface. Components never
 * touch the raw client; they subscribe to gateway state and call typed
 * request methods. Streaming turns emit ordered events so callers can resume
 * from a sequence cursor after a reconnect.
 */
export class Gateway {
  private state: GatewayState = "idle";
  private consecutiveFailures = 0;
  private readonly listeners = new Set<GatewayEvents>();

  subscribe(listener: GatewayEvents): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getState(): GatewayState {
    return this.state;
  }

  private setState(next: GatewayState) {
    if (this.state === next) return;
    this.state = next;
    for (const listener of this.listeners) listener.onState?.(next);
  }

  private noteSuccess() {
    this.consecutiveFailures = 0;
    this.setState("online");
  }

  private noteFailure(error: unknown) {
    this.consecutiveFailures += 1;
    const message = error instanceof Error ? error.message : String(error);
    for (const listener of this.listeners) listener.onError?.(message);
    // Boot-failure semantics: after repeated failed contact the shell shows
    // the failure overlay instead of a silent retry loop.
    this.setState(
      this.consecutiveFailures >= MAX_FAILURES_BEFORE_BOOT_FAILURE ? "failed" : "reconnecting",
    );
    return message;
  }

  /** Health probe used by the shell loop; resolves when the daemon answers. */
  async ping(): Promise<boolean> {
    this.setState("connecting");
    try {
      await getStewardClient().ping({});
      this.noteSuccess();
      return true;
    } catch (error) {
      this.noteFailure(error);
      return false;
    }
  }

  /** Poll loop with bounded reconnect; stops after boot-failure threshold. */
  startPolling(intervalMs = 5_000): () => void {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = async () => {
      if (stopped) return;
      if (this.state !== "failed") {
        await this.ping();
      }
      timer = setTimeout(() => void tick(), this.state === "online" ? intervalMs : RECONNECT_DELAY_MS);
    };
    void tick();
    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
    };
  }

  async listSessions(limit = 30): Promise<ChatSessionSummary[]> {
    try {
      const response = await getStewardClient().listChatSessions({ limit });
      this.noteSuccess();
      return response.sessions;
    } catch (error) {
      this.noteFailure(error);
      return [];
    }
  }

  /**
   * Resume: replay messages strictly after `sinceSequence` (0 = full). The
   * cursor is the last seen ChatMessageInfo.message_id.
   */
  async resumeSession(sessionId: string, sinceSequence: number): Promise<ChatMessageInfo[]> {
    try {
      const response = await getStewardClient().getChatSession({
        sessionId,
        sinceSequence,
      });
      this.noteSuccess();
      return response.messages;
    } catch (error) {
      this.noteFailure(error);
      return [];
    }
  }

  async activateModel(profileId: string): Promise<string> {
    const active = await getStewardClient().activateProviderProfile({ profileId });
    return active.model;
  }
}

/** Module-level singleton shared by the shell, chat, and overlays. */
export const gateway = new Gateway();

export { ChatEventKind };
