"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { stewardClient } from "@/lib/grpc";
import type { GraphNode, GraphEdge } from "@/lib/types";

interface LayoutNode {
  id: string;
  label: string;
  nodeType: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
}

interface LayoutEdge {
  source: string;
  target: string;
  relationship: string;
  weight: number;
}

/* ── Hermetica palette for graph node types ── */
const NODE_COLORS: Record<string, string> = {
  entity: "#D4AF37",
  concept: "#D0C5AF",
  action: "#F2CA50",
  agent: "#E8E2D2",
  task: "#E9C349",
  memory: "#99907C",
  default: "#4D4635",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Unknown error";
}

export default function KGViewer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [nodes, setNodes] = useState<LayoutNode[]>([]);
  const [edges, setEdges] = useState<LayoutEdge[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<LayoutNode | null>(null);
  const [filter, setFilter] = useState("");
  const [depth, setDepth] = useState(2);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const animRef = useRef<number>(0);
  const dragRef = useRef<{ id: string; ox: number; oy: number } | null>(null);
  const mousePos = useRef({ x: 0, y: 0 });

  const loadGraph = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await stewardClient.getKnowledgeGraph({
        entityFilter: filter,
        depth,
        limit: 100,
      });

      const layoutNodes: LayoutNode[] = res.nodes.map((n: GraphNode) => ({
        id: n.id,
        label: n.label,
        nodeType: n.nodeType || "entity",
        x: 200 + Math.random() * 500,
        y: 100 + Math.random() * 300,
        vx: 0,
        vy: 0,
        radius: 16 + Math.min(n.label.length * 2, 20),
      }));

      const layoutEdges: LayoutEdge[] = res.edges.map((e: GraphEdge) => ({
        source: e.sourceId,
        target: e.targetId,
        relationship: e.relationship,
        weight: e.weight || 1,
      }));

      setNodes(layoutNodes);
      setEdges(layoutEdges);

      if (layoutNodes.length === 0) {
        setError("Knowledge graph is empty. Add entities to get started.");
      }
    } catch (err: unknown) {
      setError(`Failed to load: ${errorMessage(err)}`);
      setNodes([]);
      setEdges([]);
    } finally {
      setLoading(false);
    }
  }, [filter, depth]);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => {
      void loadGraph();
    }, 0);
    return () => clearTimeout(initialLoad);
  }, [loadGraph]);

  // Force-directed layout
  useEffect(() => {
    if (nodes.length === 0) return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const W = canvas.width;
    const H = canvas.height;
    const REPULSION = 8000;
    const ATTRACTION = 0.003;
    const CENTER_GRAVITY = 0.005;
    const MIN_DIST = 40;

    let running = true;

    const simulate = () => {
      if (!running) return;

      // Reset forces
      for (const n of nodes) {
        n.vx *= 0.95;
        n.vy *= 0.95;
        n.vx += (W / 2 - n.x) * CENTER_GRAVITY;
        n.vy += (H / 2 - n.y) * CENTER_GRAVITY;
      }

      // Repulsion
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i];
          const b = nodes[j];
          const dx = a.x - b.x;
          const dy = a.y - b.y;
          const dist = Math.max(Math.sqrt(dx * dx + dy * dy), MIN_DIST);
          const force = REPULSION / (dist * dist);
          const fx = (dx / dist) * force;
          const fy = (dy / dist) * force;
          a.vx += fx;
          a.vy += fy;
          b.vx -= fx;
          b.vy -= fy;
        }
      }

      // Attraction along edges
      for (const edge of edges) {
        const s = nodes.find((n) => n.id === edge.source);
        const t = nodes.find((n) => n.id === edge.target);
        if (!s || !t) continue;
        const dx = t.x - s.x;
        const dy = t.y - s.y;
        const dist = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
        const force = (dist - 120) * ATTRACTION * edge.weight;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        s.vx += fx;
        s.vy += fy;
        t.vx -= fx;
        t.vy -= fy;
      }

      // Apply and contain
      const selected = dragRef.current
        ? nodes.find((n) => n.id === dragRef.current!.id)
        : null;
      for (const n of nodes) {
        if (n === selected) continue;
        n.x += n.vx;
        n.y += n.vy;
        n.x = Math.max(30, Math.min(W - 30, n.x));
        n.y = Math.max(30, Math.min(H - 30, n.y));
      }
      if (selected) {
        selected.x = Math.max(30, Math.min(W - 30, mousePos.current.x));
        selected.y = Math.max(30, Math.min(H - 30, mousePos.current.y));
      }

      draw();
      animRef.current = requestAnimationFrame(simulate);
    };

    const draw = () => {
      ctx.clearRect(0, 0, W, H);

      // Edges
      for (const edge of edges) {
        const s = nodes.find((n) => n.id === edge.source);
        const t = nodes.find((n) => n.id === edge.target);
        if (!s || !t) continue;

        const hl = hoveredNode === edge.source || hoveredNode === edge.target;
        ctx.strokeStyle = hl ? "#D4AF37" : "#4D4635";
        ctx.lineWidth = hl ? 2 : 0.5 + edge.weight * 0.3;
        ctx.globalAlpha = hl ? 0.8 : 0.4;
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.stroke();

        // Relationship label
        if (hl) {
          const mx = (s.x + t.x) / 2;
          const my = (s.y + t.y) / 2;
          ctx.fillStyle = "#D4AF37";
          ctx.font = "8px JetBrains Mono, monospace";
          ctx.textAlign = "center";
          ctx.fillText(edge.relationship, mx, my - 6);
        }
        ctx.globalAlpha = 1;
      }

      // Nodes
      for (const n of nodes) {
        const isHover = hoveredNode === n.id;
        const isSel = selectedNode?.id === n.id;
        const color = NODE_COLORS[n.nodeType] || NODE_COLORS.default;

        // Glow when selected/hovered
        if (isSel || isHover) {
          ctx.shadowColor = color;
          ctx.shadowBlur = isSel ? 20 : 10;
        }

        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fillStyle = isSel || isHover ? color : color + "66";
        ctx.fill();
        ctx.strokeStyle = isSel || isHover ? color : color + "33";
        ctx.lineWidth = isSel ? 2 : 1;
        ctx.stroke();
        ctx.shadowBlur = 0;

        // Label
        ctx.fillStyle = isHover ? "#F5F5F5" : "#99907C";
        ctx.font = isHover
          ? "bold 10px JetBrains Mono, monospace"
          : "9px JetBrains Mono, monospace";
        ctx.textAlign = "center";
        ctx.textBaseline = "top";
        const label = n.label.length > 20 ? n.label.slice(0, 18) + ".." : n.label;
        ctx.fillText(label, n.x, n.y + n.radius + 4);
      }
    };

    draw();
    animRef.current = requestAnimationFrame(simulate);

    return () => {
      running = false;
      cancelAnimationFrame(animRef.current);
    };
  }, [nodes, edges, hoveredNode, selectedNode]);

  const getNodeAt = useCallback(
    (x: number, y: number): LayoutNode | null => {
      for (const n of nodes) {
        const dx = x - n.x;
        const dy = y - n.y;
        if (dx * dx + dy * dy < (n.radius + 5) * (n.radius + 5)) return n;
      }
      return null;
    },
    [nodes]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      mousePos.current = { x, y };
      const node = getNodeAt(x, y);
      setHoveredNode(node?.id ?? null);
      if (canvasRef.current) {
        canvasRef.current.style.cursor = node ? "grab" : "default";
      }
    },
    [getNodeAt]
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const node = getNodeAt(x, y);
      if (node) {
        dragRef.current = { id: node.id, ox: 0, oy: 0 };
        setSelectedNode(node);
        if (canvasRef.current) canvasRef.current.style.cursor = "grabbing";
      } else {
        setSelectedNode(null);
      }
    },
    [getNodeAt]
  );

  const handleMouseUp = useCallback(() => {
    dragRef.current = null;
    if (canvasRef.current) canvasRef.current.style.cursor = "default";
  }, []);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto p-6">
        {/* Header */}
        <div className="mb-5">
          <h2 className="font-headline-lg text-headline-lg text-on-surface tracking-tight">
            Knowledge Graph
          </h2>
          <p className="text-sm text-on-surface-variant/50 mt-1">
            Explore entities and relationships in the knowledge graph
          </p>
        </div>

        {/* Controls */}
        <div className="flex items-center gap-3 mb-4 flex-wrap">
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter by label..."
            className="input-ledger min-w-[200px] flex-1"
          />
          <select
            value={depth}
            onChange={(e) => setDepth(Number(e.target.value))}
            className="bg-transparent border border-outline/20 text-on-surface text-sm px-3 py-2 outline-none focus:border-primary/50 font-mono"
          >
            {[1, 2, 3, 4, 5].map((d) => (
              <option key={d} value={d} className="bg-surface-container-low text-on-surface">Depth {d}</option>
            ))}
          </select>
          <button
            onClick={loadGraph}
            disabled={loading}
            className="btn-ghost disabled:opacity-30"
          >
            {loading ? "Loading..." : "Refresh"}
          </button>
        </div>

        {/* Canvas */}
        <div className="bg-surface-container-lowest border border-outline/20 overflow-hidden">
          {loading ? (
            <div className="flex items-center justify-center h-[450px] text-sm text-outline font-mono">
              Loading graph...
            </div>
          ) : error && nodes.length === 0 ? (
            <div className="flex items-center justify-center h-[450px] text-sm text-on-surface-variant/50 font-mono">
              {error}
            </div>
          ) : (
            <canvas
              ref={canvasRef}
              width={900}
              height={450}
              className="w-full h-[450px]"
              onMouseMove={handleMouseMove}
              onMouseDown={handleMouseDown}
              onMouseUp={handleMouseUp}
              onMouseLeave={handleMouseUp}
            />
          )}
        </div>

        {/* Legend */}
        <div className="mt-4 flex flex-wrap gap-4">
          {Object.entries(NODE_COLORS).map(([type, color]) => (
            <div key={type} className="flex items-center gap-1.5 font-label-mono text-[10px] uppercase tracking-wider text-on-surface-variant/50">
              <span className="inline-block w-2 h-2 rounded-full" style={{ backgroundColor: color }} />
              {type}
            </div>
          ))}
        </div>

        {/* Selected node info */}
        {selectedNode && (
          <div className="mt-4 border border-outline/20 p-4 card-ghost">
            <div className="flex items-center gap-3">
              <span
                className="inline-block w-3 h-3 rounded-full"
                style={{ backgroundColor: NODE_COLORS[selectedNode.nodeType] || NODE_COLORS.default }}
              />
              <div>
                <p className="text-sm text-on-surface font-medium">{selectedNode.label}</p>
                <p className="font-label-mono text-[10px] text-on-surface-variant/50 mt-0.5">
                  {selectedNode.nodeType} · {selectedNode.id.slice(0, 8)}...
                </p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
