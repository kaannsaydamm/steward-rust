import { useCallback, useEffect, useMemo, useState } from "react";
import { RefreshCw, Search } from "lucide-react";
import { Button } from "@nous-research/ui/ui/components/button";
import { Card, CardContent } from "@nous-research/ui/ui/components/card";
import { Input } from "@nous-research/ui/ui/components/input";
import { Badge } from "@nous-research/ui/ui/components/badge";
import { Spinner } from "@nous-research/ui/ui/components/spinner";
import { api } from "@/lib/api";
import { usePageHeader } from "@/contexts/usePageHeader";

interface KgNode {
  id: string;
  label: string;
  type: string;
}

interface KgEdge {
  id: string;
  source: string;
  target: string;
  relationship: string;
  weight: number;
}

interface KgGraph {
  nodes: KgNode[];
  edges: KgEdge[];
}

/** Knowledge graph viewer (supersedes the legacy web-ui KGViewer):
 *  node list + adjacency rendering straight from the daemon's KG bridge. */
export default function KgPage() {
  const [graph, setGraph] = useState<KgGraph>({ nodes: [], edges: [] });
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const load = useCallback(async (q?: string) => {
    setLoading(true);
    setError("");
    try {
      const data = q
        ? await api.stewardKgQuery({ query: q, max_hops: 2, limit: 80 })
        : await api.stewardKgGraph(2);
      setGraph({ nodes: data.nodes ?? [], edges: data.edges ?? [] });
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  usePageHeader().setAfterTitle("Knowledge Graph");

  const neighbors = useMemo(() => {
    const byId = new Map(graph.nodes.map((n) => [n.id, n]));
    return graph.nodes.map((n: KgNode) => ({
      ...n,
      links: graph.edges
        .filter((e: KgEdge) => e.source === n.id || e.target === n.id)
        .map((e: KgEdge) => {
          const other = e.source === n.id ? byId.get(e.target) : byId.get(e.source);
          return `${e.relationship} → ${other?.label ?? e.target}`;
        }),
    }));
  }, [graph]);

  return (
    <div className="flex flex-1 flex-col gap-4 p-4">
      <div className="flex items-center gap-2">
        <Input
          className="max-w-md"
          placeholder="Search an entity…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && query.trim()) void load(query.trim());
          }}
        />
        <Button
          size="sm"
          onClick={() => void load(query.trim() || undefined)}
        >
          <Search className="h-3.5 w-3.5" /> Search
        </Button>
        <Button size="sm" ghost onClick={() => void load()}>
          <RefreshCw className="h-3.5 w-3.5" /> Reset
        </Button>
      </div>

      {loading && <Spinner className="mx-auto my-8" />}
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!loading && !error && neighbors.length === 0 && (
        <p className="my-8 text-center text-sm text-muted-foreground">
          No graph entities yet — they appear as the agent learns from sessions.
        </p>
      )}

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {neighbors.map((node: (typeof neighbors)[number]) => (
          <Card key={node.id}>
            <CardContent className="flex flex-col gap-2 py-3">
              <div className="flex items-center gap-2">
                <Badge>{node.type}</Badge>
                <span className="truncate text-sm font-medium">{node.label}</span>
              </div>
              {node.links.length > 0 && (
                <ul className="flex flex-col gap-1 text-xs text-muted-foreground">
                  {node.links.slice(0, 6).map((link: string, i: number) => (
                    <li key={i} className="truncate">
                      {link}
                    </li>
                  ))}
                  {node.links.length > 6 && (
                    <li className="text-text-tertiary">+{node.links.length - 6} more…</li>
                  )}
                </ul>
              )}
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
