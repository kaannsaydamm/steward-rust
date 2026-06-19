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
