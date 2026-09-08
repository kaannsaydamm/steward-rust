"""Steward daemon bridge routes — REST proxy to the daemon's /api/kg
knowledge-graph surface so the dashboard KG viewer needs no direct gRPC or
wire-port knowledge. The daemon owns graph truth; this is a thin pass-through.
"""

from __future__ import annotations

import json
import os
import urllib.request

from fastapi import APIRouter, HTTPException, Query

router = APIRouter()


def _wire_base() -> str:
    port_file = os.path.expanduser(os.path.join("~", ".steward", "wire-port"))
    try:
        port = open(port_file, encoding="utf-8").read().strip()
    except OSError as exc:  # pragma: no cover - daemon not installed
        raise HTTPException(status_code=503, detail=f"wire port unreadable: {exc}") from exc
    return f"http://127.0.0.1:{port}"


def _fetch_json(url: str, body: dict | None = None) -> dict:
    try:
        if body is None:
            req = urllib.request.Request(url)
        else:
            req = urllib.request.Request(
                url,
                data=json.dumps(body).encode("utf-8"),
                headers={"Content-Type": "application/json"},
            )
        with urllib.request.urlopen(req, timeout=30) as res:
            return json.loads(res.read().decode("utf-8"))
    except Exception as exc:  # noqa: BLE001 — surface any daemon failure as 502
        raise HTTPException(status_code=502, detail=str(exc)) from exc


@router.get("/api/steward/kg")
def kg_graph(
    depth: int = Query(2, ge=0, le=6),
    filter: str = Query(""),
) -> dict:
    """Knowledge graph snapshot (nodes + edges) from the daemon."""
    return _fetch_json(f"{_wire_base()}/api/kg/graph?depth={depth}&filter={filter}")


@router.post("/api/steward/kg/query")
def kg_query(body: dict) -> dict:
    """Subgraph around a query entity from the daemon."""
    return _fetch_json(
        f"{_wire_base()}/api/kg/query",
        {
            "query": str(body.get("query", "")),
            "max_hops": int(body.get("max_hops", 2)),
            "limit": int(body.get("limit", 50)),
        },
    )
