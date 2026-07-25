#!/usr/bin/env python3
"""Executable seed for immutable multi-agent evidence coordination.

This is intentionally finite and tiny. It tests authority separation, content
identity, stale-evidence rejection, conflict surfacing, and promotion policy.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
from typing import Any, Literal

Authority = Literal["agent", "verifier", "proof_kernel", "human_policy"]
Status = Literal["proposal", "observed", "validated", "proved", "blocked"]


def digest(value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()[:20]


@dataclass(frozen=True)
class Node:
    kind: str
    payload: dict[str, Any]
    authority: Authority
    status: Status

    @property
    def id(self) -> str:
        return f"eg_{digest(asdict(self))}"


@dataclass(frozen=True)
class Edge:
    source: str
    relation: Literal["supports", "refutes", "repairs", "checks", "conflicts"]
    target: str


class EvidenceGraph:
    ALLOWED_STATUS = {
        "agent": {"proposal", "observed"},
        "verifier": {"observed", "validated", "blocked"},
        "proof_kernel": {"observed", "validated", "proved", "blocked"},
        "human_policy": {"observed", "validated", "proved", "blocked"},
    }

    def __init__(self) -> None:
        self.nodes: dict[str, Node] = {}
        self.edges: set[Edge] = set()

    def add(self, node: Node) -> str:
        if node.status not in self.ALLOWED_STATUS[node.authority]:
            raise PermissionError(
                f"{node.authority} cannot publish status {node.status}"
            )
        self.nodes.setdefault(node.id, node)
        return node.id

    def link(self, source: str, relation: Edge.__annotations__["relation"], target: str) -> None:
        if source not in self.nodes or target not in self.nodes:
            raise KeyError("edges require existing immutable nodes")
        self.edges.add(Edge(source, relation, target))

    def incoming(self, target: str, relation: str) -> list[Node]:
        return [
            self.nodes[e.source]
            for e in self.edges
            if e.target == target and e.relation == relation
        ]

    def conflicting_claims(self) -> list[tuple[str, str]]:
        claims = [
            (node_id, node)
            for node_id, node in self.nodes.items()
            if node.kind == "claim"
        ]
        conflicts: list[tuple[str, str]] = []
        for i, (left_id, left) in enumerate(claims):
            for right_id, right in claims[i + 1 :]:
                if (
                    left.payload.get("subject") == right.payload.get("subject")
                    and left.payload.get("predicate") == right.payload.get("predicate")
                    and left.payload.get("verdict") != right.payload.get("verdict")
                ):
                    conflicts.append(tuple(sorted((left_id, right_id))))
        return sorted(set(conflicts))


def promotion_verdict(
    graph: EvidenceGraph,
    proposal_id: str,
    *,
    current_snapshot: str,
    intent_id: str,
) -> dict[str, Any]:
    proposal = graph.nodes[proposal_id]
    supports = graph.incoming(proposal_id, "supports")
    refutations = graph.incoming(proposal_id, "refutes")

    required = {"exact_replay", "neighborhood", "mutation_challenge", "clean_parity"}
    valid_supports = {
        node.payload["check"]
        for node in supports
        if node.authority in {"verifier", "proof_kernel"}
        and node.status in {"validated", "proved"}
        and node.payload.get("snapshot") == current_snapshot
        and node.payload.get("intent") == intent_id
    }
    protected_refutations = [
        node
        for node in refutations
        if node.status in {"validated", "proved", "blocked"}
        and node.payload.get("protected_intent_change") is True
    ]
    reasons: list[str] = []
    if proposal.payload.get("snapshot") != current_snapshot:
        reasons.append("proposal targets stale snapshot")
    if proposal.payload.get("intent") != intent_id:
        reasons.append("proposal targets different intent")
    missing = sorted(required - valid_supports)
    if missing:
        reasons.append(f"missing evidence: {', '.join(missing)}")
    if protected_refutations:
        reasons.append("protected intent changed")
    return {
        "accepted": not reasons,
        "reasons": reasons,
        "valid_supports": sorted(valid_supports),
    }


def run() -> dict[str, Any]:
    graph = EvidenceGraph()
    snapshot = "ws_current"
    old_snapshot = "ws_old"
    intent = "in_ack_durable"

    unauthorized_promotion_rejected = False
    try:
        graph.add(Node("claim", {"subject": "patch", "predicate": "safe", "verdict": True}, "agent", "validated"))
    except PermissionError:
        unauthorized_promotion_rejected = True
    assert unauthorized_promotion_rejected

    safe_patch = Node(
        "patch_proposal",
        {"patch": "move Ack after Sync", "snapshot": snapshot, "intent": intent},
        "agent",
        "proposal",
    )
    safe_id = graph.add(safe_patch)
    # A second agent independently proposes byte-identical content.
    duplicate_id = graph.add(safe_patch)
    assert duplicate_id == safe_id and len(graph.nodes) == 1

    for check in ("exact_replay", "neighborhood", "mutation_challenge"):
        ev = Node(
            "verification_evidence",
            {"check": check, "snapshot": snapshot, "intent": intent, "passed": True},
            "verifier",
            "validated",
        )
        ev_id = graph.add(ev)
        graph.link(ev_id, "supports", safe_id)

    # Stale clean-parity evidence must not complete the promotion envelope.
    stale = Node(
        "verification_evidence",
        {"check": "clean_parity", "snapshot": old_snapshot, "intent": intent, "passed": True},
        "verifier",
        "validated",
    )
    stale_id = graph.add(stale)
    graph.link(stale_id, "supports", safe_id)
    before_fresh = promotion_verdict(graph, safe_id, current_snapshot=snapshot, intent_id=intent)
    assert before_fresh["accepted"] is False
    assert any("clean_parity" in reason for reason in before_fresh["reasons"])

    fresh = Node(
        "verification_evidence",
        {"check": "clean_parity", "snapshot": snapshot, "intent": intent, "passed": True},
        "proof_kernel",
        "proved",
    )
    fresh_id = graph.add(fresh)
    graph.link(fresh_id, "supports", safe_id)
    accepted = promotion_verdict(graph, safe_id, current_snapshot=snapshot, intent_id=intent)
    assert accepted["accepted"] is True

    gaming_patch = Node(
        "patch_proposal",
        {"patch": "remove Crash from fault model", "snapshot": snapshot, "intent": intent},
        "agent",
        "proposal",
    )
    gaming_id = graph.add(gaming_patch)
    intent_refutation = Node(
        "semantic_diff",
        {
            "protected_intent_change": True,
            "field": "faults",
            "relation": "contracted",
            "snapshot": snapshot,
            "intent": intent,
        },
        "verifier",
        "blocked",
    )
    refute_id = graph.add(intent_refutation)
    graph.link(refute_id, "refutes", gaming_id)
    gaming_verdict = promotion_verdict(graph, gaming_id, current_snapshot=snapshot, intent_id=intent)
    assert gaming_verdict["accepted"] is False
    assert "protected intent changed" in gaming_verdict["reasons"]

    positive_claim = Node(
        "claim",
        {"subject": safe_id, "predicate": "AckImpliesDurable", "verdict": True},
        "verifier",
        "validated",
    )
    negative_claim = Node(
        "claim",
        {"subject": safe_id, "predicate": "AckImpliesDurable", "verdict": False},
        "verifier",
        "observed",
    )
    pos_id = graph.add(positive_claim)
    neg_id = graph.add(negative_claim)
    conflicts = graph.conflicting_claims()
    assert tuple(sorted((pos_id, neg_id))) in conflicts
    graph.link(pos_id, "conflicts", neg_id)

    return {
        "node_count": len(graph.nodes),
        "edge_count": len(graph.edges),
        "duplicate_proposal_deduplicated": duplicate_id == safe_id,
        "unauthorized_promotion_rejected": unauthorized_promotion_rejected,
        "stale_evidence_rejected": before_fresh["accepted"] is False,
        "accepted_after_complete_fresh_envelope": accepted["accepted"],
        "accepted_supports": accepted["valid_supports"],
        "intent_gaming_blocked": gaming_verdict["accepted"] is False,
        "intent_gaming_reasons": gaming_verdict["reasons"],
        "conflict_count": len(conflicts),
        "conflicts_surfaced": len(conflicts) == 1,
    }


if __name__ == "__main__":
    print(json.dumps(run(), indent=2, sort_keys=True))
