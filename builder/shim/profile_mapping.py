"""Maps autogen team component configs to Steward AgentProfile YAML.

The builder edits a component tree (team → participants → model client…).
This module extracts the agent-relevant slice and renders an AgentProfile
document the Steward daemon can validate and persist. It never talks to the
daemon itself — persistence happens in `app.py` via gRPC.

ADAPTER code — docs/OSS_PORT_MAP.md A2.
"""

from __future__ import annotations

import re
from typing import Any, Dict, List

_SAFE_ID = re.compile(r"[^a-z0-9_-]+")


def _slug(text: str, fallback: str) -> str:
    slug = _SAFE_ID.sub("-", (text or "").lower()).strip("-")
    return slug or fallback


def _participants(component: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Collects participant agents from either a wrapped team component or a
    bare config dict."""
    config = component.get("config") or component
    participants = config.get("participants") or []
    if not participants:
        # Single-agent component: treat the component itself as the participant.
        agent_type = component.get("type") or config.get("agent_type", "")
        if "AssistantAgent" in str(agent_type) or config.get("system_message"):
            participants = [component]
    return participants


def _first_model(component: Dict[str, Any]) -> Dict[str, Any]:
    for participant in _participants(component):
        config = participant.get("config") or {}
        model_client = config.get("model_client")
        if isinstance(model_client, dict):
            return model_client
    return (component.get("config") or {}).get("model_client") or {}


def _model_policy(component: Dict[str, Any]) -> str:
    """Steward ModelPolicy hint: explicit provider model name when present,
    else auto (the Steward router decides)."""
    model = _first_model(component)
    model_name = str(model.get("model") or model.get("model_name") or "")
    return model_name if model_name else "auto"


def _system_instructions(participants: List[Dict[str, Any]]) -> str:
    chunks: List[str] = []
    for participant in participants:
        config = participant.get("config") or {}
        message = config.get("system_message")
        if isinstance(message, str) and message.strip():
            label = participant.get("label") or config.get("name") or "agent"
            chunks.append(f"[{label}]\n{message.strip()}")
    return "\n\n".join(chunks)


def team_type(component: Dict[str, Any]) -> str:
    component_type = str(component.get("type") or "")
    if "RoundRobin" in component_type:
        return "round_robin"
    if "Selector" in component_type:
        return "selector"
    if "Swarm" in component_type or "Handoff" in component_type:
        return "handoff"
    return "round_robin"


def team_topology_comment(component: Dict[str, Any]) -> str:
    members = [
        (p.get("label") or (p.get("config") or {}).get("name") or "agent")
        for p in _participants(component)
    ]
    return (
        f"# team topology: {team_type(component)}; members: {', '.join(members) or 'none'}"
    )


def component_to_agent_profile_yaml(component: Dict[str, Any]) -> str:
    """Renders the builder's team component as an AgentProfile YAML doc.

    One profile per team: persona instructions concatenated from participant
    agents, subagents listing member labels (delegation seeds for the kernel
    scheduler), model policy from the first model client.
    """
    label = _config_to_label(component)
    participants = _participants(component)
    instructions = _system_instructions(participants)
    primary = participants[0] if participants else {}
    primary_config = primary.get("config") or {}
    role = str(primary.get("label") or primary_config.get("name") or "team")
    description = str(
        (component.get("config") or {}).get("description")
        or primary_config.get("description")
        or ""
    ).replace('"', "'")

    subagents = [
        _slug(str(p.get("label") or (p.get("config") or {}).get("name") or ""), "member")
        for p in participants[1:]
    ]

    lines = [
        team_topology_comment(component),
        "schema_version: 1",
        f"id: {_slug(label, 'team')}",
        f'title: "{label}"',
    ]
    if description:
        lines.append(f'description: "{description}"')
    model_policy = _model_policy(component)
    if model_policy == "auto":
        lines.append("model: auto")
    else:
        lines.append("model: auto  # explicit model hint: " + model_policy)
    lines += [
        "context:",
        "  mode: fresh",
        "tools:",
        "  discovery: false",
        "  allow_effects: [filesystem_read]",
        "workspace:",
        "  mode: read_write",
        "persona:",
        f'  role: "{role}"',
        f'  system_instructions: "{instructions.replace(chr(34), chr(39))}"',
    ]
    if subagents:
        lines.append("subagents: [" + ", ".join(subagents) + "]")
    lines.append("template: false")
    return "\n".join(lines) + "\n"


def agent_profile_error(yaml_text: str) -> str:
    """Best-effort client-side validation mirroring the daemon's checks; the
    authoritative validation happens in the daemon's AgentProfile::validate."""
    for line in yaml_text.splitlines():
        if line.startswith("id: "):
            id_value = line[4:].strip()
            if not re.fullmatch(r"[a-z0-9_-]+", id_value):
                return f"profile id '{id_value}' is invalid: use lowercase ASCII, digits, '-'"
            return ""
    return "profile document missing id"


def _config_to_label(component: Dict[str, Any]) -> str:
    """Team label from the component envelope or its config."""
    label = component.get("label")
    if label:
        return str(label)
    config = component.get("config") or {}
    return str(config.get("label") or config.get("name") or "")
