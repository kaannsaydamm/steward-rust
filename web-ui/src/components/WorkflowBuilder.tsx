"use client";

import { PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { stewardClient } from "@/lib/grpc";
import {
  WorkflowDefinition,
  type WorkflowConnection,
  type WorkflowNode,
} from "@/lib/proto/steward";

interface Props {
  onRunFinished: () => Promise<void>;
}

interface DragState {
  nodeId: string;
  offsetX: number;
  offsetY: number;
}

const EMPTY_DEFINITION: WorkflowDefinition = {
  definitionId: "",
  name: "Software delivery",
  description: "Plan, implement, and review a software task.",
  nodes: [
    node("plan", "Plan", "Create a concise implementation plan.", 40, 70, false),
    node("build", "Build", "Implement the approved task and use tools when needed.", 310, 70, true),
    node("review", "Review", "Review the result, run checks, and report concrete findings.", 580, 70, true),
  ],
  connections: [connection("plan", "build"), connection("build", "review")],
  createdAt: 0,
  updatedAt: 0,
};

export default function WorkflowBuilder({ onRunFinished }: Props) {
  const [definitions, setDefinitions] = useState<WorkflowDefinition[]>([]);
  const [draft, setDraft] = useState<WorkflowDefinition>(EMPTY_DEFINITION);
  const [selectedNodeId, setSelectedNodeId] = useState("plan");
  const [sourceId, setSourceId] = useState("plan");
  const [targetId, setTargetId] = useState("build");
  const [mode, setMode] = useState<"visual" | "json">("visual");
  const [jsonText, setJsonText] = useState(JSON.stringify(EMPTY_DEFINITION, null, 2));
  const [drag, setDrag] = useState<DragState | null>(null);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const canvasRef = useRef<HTMLDivElement>(null);

  const loadDefinitions = useCallback(async () => {
    const response = await stewardClient.listWorkflowDefinitions({});
    setDefinitions(response.definitions);
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadDefinitions().catch((reason) => setStatus(String(reason)));
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadDefinitions]);

  const selectedNode = useMemo(
    () => draft.nodes.find((item) => item.nodeId === selectedNodeId),
    [draft.nodes, selectedNodeId],
  );

  function updateDraft(next: WorkflowDefinition) {
    setDraft(next);
    setJsonText(JSON.stringify(next, null, 2));
  }

  function chooseDefinition(definition: WorkflowDefinition) {
    updateDraft(definition);
    setSelectedNodeId(definition.nodes[0]?.nodeId ?? "");
    setStatus("");
  }

  function addNode() {
    let index = draft.nodes.length + 1;
    while (draft.nodes.some((item) => item.nodeId === `step_${index}`)) index += 1;
    const next = node(`step_${index}`, `Step ${index}`, "Describe the work for this step.", 50 + index * 35, 180 + index * 24, true);
    updateDraft({ ...draft, nodes: [...draft.nodes, next] });
    setSelectedNodeId(next.nodeId);
  }

  function removeSelectedNode() {
    if (!selectedNode) return;
    const nodes = draft.nodes.filter((item) => item.nodeId !== selectedNode.nodeId);
    const connections = draft.connections.filter(
      (edge) => edge.sourceNodeId !== selectedNode.nodeId && edge.targetNodeId !== selectedNode.nodeId,
    );
    updateDraft({ ...draft, nodes, connections });
    setSelectedNodeId(nodes[0]?.nodeId ?? "");
  }

  function connectNodes() {
    if (!sourceId || !targetId || sourceId === targetId) return;
    const duplicate = draft.connections.some(
      (edge) => edge.sourceNodeId === sourceId && edge.targetNodeId === targetId,
    );
    if (!duplicate) updateDraft({ ...draft, connections: [...draft.connections, connection(sourceId, targetId)] });
  }

  function beginDrag(event: PointerEvent<HTMLButtonElement>, item: WorkflowNode) {
    const bounds = event.currentTarget.getBoundingClientRect();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDrag({ nodeId: item.nodeId, offsetX: event.clientX - bounds.left, offsetY: event.clientY - bounds.top });
    setSelectedNodeId(item.nodeId);
  }

  function moveDrag(event: PointerEvent<HTMLDivElement>) {
    const canvas = canvasRef.current;
    if (!drag || !canvas) return;
    const bounds = canvas.getBoundingClientRect();
    const x = Math.max(8, Math.min(bounds.width - 202, event.clientX - bounds.left - drag.offsetX));
    const y = Math.max(8, Math.min(bounds.height - 112, event.clientY - bounds.top - drag.offsetY));
    setDraft((current) => ({
      ...current,
      nodes: current.nodes.map((item) =>
        item.nodeId === drag.nodeId ? { ...item, positionX: Math.round(x), positionY: Math.round(y) } : item,
      ),
    }));
  }

  function applyJson() {
    try {
      const parsed: unknown = JSON.parse(jsonText);
      const definition = WorkflowDefinition.fromJSON(parsed);
      updateDraft(definition);
      setSelectedNodeId(definition.nodes[0]?.nodeId ?? "");
      setStatus("JSON applied.");
    } catch (reason) {
      setStatus(reason instanceof Error ? reason.message : String(reason));
    }
  }

  async function save() {
    setBusy(true);
    try {
      const saved = await stewardClient.saveWorkflowDefinition({ definition: draft });
      updateDraft(saved);
      await loadDefinitions();
      setStatus("Workflow definition saved.");
    } catch (reason) {
      setStatus(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function run() {
    setBusy(true);
    try {
      const saved = await stewardClient.saveWorkflowDefinition({ definition: draft });
      updateDraft(saved);
      const stream = stewardClient.startWorkflow({
        title: saved.name,
        description: saved.description,
        targetRepo: "",
        files: [],
        constraints: {},
        definitionId: saved.definitionId,
      });
      for await (const event of stream) setStatus(`${event.message}: ${event.detail}`);
      await onRunFinished();
    } catch (reason) {
      setStatus(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <select className="border border-outline-variant/50 bg-surface-container px-3 py-2 text-xs" value={draft.definitionId} onChange={(event) => {
          const selected = definitions.find((item) => item.definitionId === event.target.value);
          if (selected) chooseDefinition(selected);
        }}>
          <option value="">Unsaved definition</option>
          {definitions.map((item) => <option key={item.definitionId} value={item.definitionId}>{item.name}</option>)}
        </select>
        <button className="btn-ghost" onClick={() => chooseDefinition({ ...EMPTY_DEFINITION, nodes: [...EMPTY_DEFINITION.nodes], connections: [...EMPTY_DEFINITION.connections] })}>New</button>
        <button className="btn-ghost" onClick={addNode}>Add node</button>
        <button className="btn-ghost" onClick={() => setMode(mode === "visual" ? "json" : "visual")}>{mode === "visual" ? "JSON" : "Visual"}</button>
        <span className="flex-1" />
        <button className="btn-ghost" disabled={busy} onClick={() => void save()}>Save</button>
        <button className="btn-ghost border-primary/60 text-primary" disabled={busy} onClick={() => void run()}>{busy ? "Running..." : "Run graph"}</button>
      </div>
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_280px]">
        {mode === "visual" ? (
          <div
            ref={canvasRef}
            onPointerMove={moveDrag}
            onPointerUp={() => { setDrag(null); setJsonText(JSON.stringify(draft, null, 2)); }}
            className="relative h-[520px] overflow-hidden border border-outline-variant/40 bg-[radial-gradient(circle_at_center,rgba(212,175,55,0.08)_1px,transparent_1px)] [background-size:24px_24px]"
          >
            <svg className="pointer-events-none absolute inset-0 h-full w-full" aria-hidden="true">
              {draft.connections.map((edge) => {
                const source = draft.nodes.find((item) => item.nodeId === edge.sourceNodeId);
                const target = draft.nodes.find((item) => item.nodeId === edge.targetNodeId);
                if (!source || !target) return null;
                return <line key={`${edge.sourceNodeId}-${edge.targetNodeId}`} x1={source.positionX + 190} y1={source.positionY + 48} x2={target.positionX} y2={target.positionY + 48} stroke="#D4AF37" strokeOpacity="0.55" strokeWidth="2" />;
              })}
            </svg>
            {draft.nodes.map((item) => (
              <button
                key={item.nodeId}
                onPointerDown={(event) => beginDrag(event, item)}
                style={{ left: item.positionX, top: item.positionY }}
                className={`absolute h-24 w-48 cursor-grab border p-3 text-left active:cursor-grabbing ${selectedNodeId === item.nodeId ? "border-primary bg-primary/10" : "border-outline-variant/60 bg-surface-container"}`}
              >
                <span className="block font-mono text-[10px] uppercase tracking-widest text-primary">{item.agentId || "operator"}</span>
                <span className="mt-1 block truncate text-sm font-medium">{item.title}</span>
                <span className="mt-2 block truncate text-[11px] text-outline">{item.instruction}</span>
              </button>
            ))}
          </div>
        ) : (
          <div className="space-y-3">
            <textarea value={jsonText} onChange={(event) => setJsonText(event.target.value)} spellCheck={false} className="h-[480px] w-full resize-none border border-outline-variant/40 bg-surface-container-low p-4 font-mono text-xs leading-5 outline-none focus:border-primary/60" />
            <button className="btn-ghost" onClick={applyJson}>Apply JSON</button>
          </div>
        )}
        <aside className="card-ghost space-y-4 p-4">
          <Field label="Workflow name" value={draft.name} onChange={(name) => updateDraft({ ...draft, name })} />
          <Field label="Goal" value={draft.description} onChange={(description) => updateDraft({ ...draft, description })} multiline />
          {selectedNode && <>
            <hr className="border-outline-variant/30" />
            <Field label="Node ID" value={selectedNode.nodeId} onChange={() => undefined} disabled />
            <Field label="Title" value={selectedNode.title} onChange={(title) => updateNode(draft, selectedNode.nodeId, { title }, updateDraft)} />
            <Field label="Agent" value={selectedNode.agentId} onChange={(agentId) => updateNode(draft, selectedNode.nodeId, { agentId }, updateDraft)} />
            <Field label="Instruction" value={selectedNode.instruction} onChange={(instruction) => updateNode(draft, selectedNode.nodeId, { instruction }, updateDraft)} multiline />
            <label className="flex items-center gap-2 text-xs text-outline"><input type="checkbox" checked={selectedNode.allowTools} onChange={(event) => updateNode(draft, selectedNode.nodeId, { allowTools: event.target.checked }, updateDraft)} /> Allow governed tools</label>
            <button className="btn-ghost border-error/40 text-error" onClick={removeSelectedNode}>Remove node</button>
          </>}
          <hr className="border-outline-variant/30" />
          <p className="font-mono text-[10px] uppercase tracking-widest text-primary">Connect nodes</p>
          <select className="w-full border border-outline-variant/40 bg-surface-container p-2 text-xs" value={sourceId} onChange={(event) => setSourceId(event.target.value)}>{draft.nodes.map((item) => <option key={item.nodeId} value={item.nodeId}>{item.title}</option>)}</select>
          <select className="w-full border border-outline-variant/40 bg-surface-container p-2 text-xs" value={targetId} onChange={(event) => setTargetId(event.target.value)}>{draft.nodes.map((item) => <option key={item.nodeId} value={item.nodeId}>{item.title}</option>)}</select>
          <button className="btn-ghost w-full" onClick={connectNodes}>Connect</button>
        </aside>
      </div>
      {status && <p role="status" className="border-l-2 border-primary px-3 py-2 text-sm text-on-surface">{status}</p>}
    </div>
  );
}

function node(nodeId: string, title: string, instruction: string, positionX: number, positionY: number, allowTools: boolean): WorkflowNode {
  return { nodeId, title, instruction, agentId: title.toLowerCase(), allowTools, positionX, positionY };
}

function connection(sourceNodeId: string, targetNodeId: string): WorkflowConnection {
  return { sourceNodeId, targetNodeId };
}

function updateNode(draft: WorkflowDefinition, nodeId: string, change: Partial<WorkflowNode>, update: (next: WorkflowDefinition) => void) {
  update({ ...draft, nodes: draft.nodes.map((item) => item.nodeId === nodeId ? { ...item, ...change } : item) });
}

function Field({ label, value, onChange, multiline = false, disabled = false }: { label: string; value: string; onChange: (value: string) => void; multiline?: boolean; disabled?: boolean }) {
  const className = "mt-1 w-full border border-outline-variant/40 bg-surface-container-low p-2 text-xs outline-none focus:border-primary/60 disabled:opacity-50";
  return <label className="block text-xs text-outline">{label}{multiline ? <textarea rows={3} value={value} onChange={(event) => onChange(event.target.value)} className={className} /> : <input disabled={disabled} value={value} onChange={(event) => onChange(event.target.value)} className={className} />}</label>;
}
