export { stewardClient } from "./grpc";

// Re-export common types from proto for convenience
export type {
  PingRequest,
  PingResponse,
  RunPluginRequest,
  RunPluginResponse,
  ExecuteTaskRequest,
  ExecuteTaskResponse,
  MemoryEntry,
  StoreMemoryRequest,
  StoreMemoryResponse,
  RecallMemoryRequest,
  RecallMemoryResponse,
  GraphNode,
  GraphEdge,
  GraphQueryRequest,
  GraphQueryResponse,
  GetKnowledgeGraphRequest,
  GetKnowledgeGraphResponse,
  ApprovalRequest,
  WorkflowEvent,
  StartWorkflowRequest,
  GetWorkflowStatusRequest,
  WorkflowStatus,
  ListWorkflowsRequest,
  ListWorkflowsResponse,
  CancelWorkflowRequest,
  CancelWorkflowResponse,
  ApprovePlanRequest,
  ApprovePlanResponse,
  AgentInfo,
  ListAgentsRequest,
  ListAgentsResponse,
  GetAgentLogRequest,
  AgentLogEntry,
  ToolInfo,
  SkillInfo,
  McpAdapterInfo,
  ToolInvocationInfo,
  MaintenanceStatus,
  CronJobInfo,
} from "./proto/steward";

/**
 * Helper: call a server-streaming RPC and collect all events into an array.
 * The nice-grpc-web streaming returns an AsyncIterable.
 */
export async function collectStream<T>(
  iterable: AsyncIterable<T>
): Promise<T[]> {
  const items: T[] = [];
  for await (const item of iterable) {
    items.push(item);
  }
  return items;
}

/**
 * Phase labels for display
 */
export const PHASE_LABELS: Record<number, string> = {
  0: "Unknown",
  1: "Discovery",
  2: "Context",
  3: "Research",
  4: "Intake",
  5: "Orchestrate",
  6: "Plan",
  7: "Awaiting Approval",
  8: "Executing",
  9: "Validating",
  10: "Completed",
  11: "Failed",
  12: "Cancelled",
};

export const PHASE_COLORS: Record<number, string> = {
  0: "text-gray-500",
  1: "text-blue-400",
  2: "text-cyan-400",
  3: "text-purple-400",
  4: "text-indigo-400",
  5: "text-yellow-400",
  6: "text-orange-400",
  7: "text-amber-400",
  8: "text-green-400",
  9: "text-teal-400",
  10: "text-green-500",
  11: "text-red-500",
  12: "text-gray-500",
};

export const MODE_LABELS: Record<number, string> = {
  0: "Parallel",
  1: "Sequential",
  2: "Hybrid",
};

/**
 * Formats a unix-seconds timestamp as a short relative time (e.g. "7m ago", "2h ago", "18d ago").
 */
export function timeAgo(unixSeconds: number): string {
  if (!unixSeconds) return "";
  const deltaSeconds = Math.max(0, Date.now() / 1000 - unixSeconds);
  const steps: [number, string][] = [
    [60, "s"],
    [60, "m"],
    [24, "h"],
    [7, "d"],
    [4.345, "w"],
    [12, "mo"],
    [Number.POSITIVE_INFINITY, "y"],
  ];
  let value = deltaSeconds;
  for (const [divisor, unit] of steps) {
    if (value < divisor || !Number.isFinite(divisor)) {
      return `${Math.max(1, Math.floor(value))}${unit} ago`;
    }
    value /= divisor;
  }
  return "just now";
}

/**
 * Formats a whole number of seconds as a short interval label (e.g. "5m", "1h", "1d").
 */
export function formatInterval(seconds: number): string {
  if (seconds % 86_400 === 0) return `${seconds / 86_400}d`;
  if (seconds % 3_600 === 0) return `${seconds / 3_600}h`;
  if (seconds % 60 === 0) return `${seconds / 60}m`;
  return `${seconds}s`;
}
