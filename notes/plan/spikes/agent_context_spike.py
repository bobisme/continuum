#!/usr/bin/env python3
from __future__ import annotations
from dataclasses import dataclass, asdict
import json
from typing import Iterable

@dataclass(frozen=True)
class Event:
    id: str
    kind: str
    owner: str
    causes: tuple[str, ...]
    key: str | None = None
    value: int | None = None


def build_trace(noise_count: int = 196) -> list[Event]:
    noise = [Event(f"n{i:03d}", "NoiseWrite", f"noise-{i%7}", (), f"k{i}", i) for i in range(noise_count)]
    core = [
        Event("e_submit", "WriteSubmitted", "writer", ()),
        Event("e_reserve", "ReplyReserved", "writer", ("e_submit",)),
        Event("e_cancel", "CancelRequested", "supervisor", ("e_reserve",)),
        Event("e_publish", "ReplyPublished", "finalizer", ("e_cancel",)),
    ]
    # Interleave the semantic core with irrelevant but legal events.
    return noise[:50] + core[:1] + noise[50:100] + core[1:2] + noise[100:150] + core[2:3] + noise[150:] + core[3:]


def replay(events: Iterable[Event]) -> tuple[bool, dict[str, bool]]:
    seen: set[str] = set()
    state = {"submitted": False, "reserved": False, "cancelled": False, "durable": False, "acked": False}
    valid = True
    for e in events:
        if any(c not in seen for c in e.causes):
            valid = False
            break
        seen.add(e.id)
        if e.kind == "WriteSubmitted": state["submitted"] = True
        elif e.kind == "ReplyReserved": state["reserved"] = state["submitted"]
        elif e.kind == "CancelRequested": state["cancelled"] = True
        elif e.kind == "SyncCompleted": state["durable"] = True
        elif e.kind == "ReplyPublished": state["acked"] = state["reserved"]
    return valid and state["acked"] and not state["durable"], state


def greedy_minimize(trace: list[Event]) -> list[Event]:
    current = trace[:]
    changed = True
    while changed:
        changed = False
        for e in current[:]:
            trial = [x for x in current if x.id != e.id]
            violation, _ = replay(trial)
            if violation:
                current = trial
                changed = True
                break
    return current


def run() -> dict:
    trace = build_trace()
    violated, raw_state = replay(trace)
    assert violated
    core = greedy_minimize(trace)
    core_violated, core_state = replay(core)
    assert core_violated
    assert [e.id for e in core] == ["e_submit", "e_reserve", "e_cancel", "e_publish"]
    for i in range(len(core)):
        assert not replay(core[:i] + core[i+1:])[0]

    raw_payload = json.dumps([asdict(e) for e in trace], separators=(",", ":"))
    pack = {
        "question": "why did AckImpliesDurable fail?",
        "verdict": "refuted",
        "causal_core": [asdict(e) for e in core],
        "abstract_delta": {"acknowledged": "false -> true", "durable": "false"},
        "missing_order": "SyncCompleted -> ReplyPublished",
        "omissions": [{"kind": "observer-independent event", "count": len(trace) - len(core), "expandable": True}],
        "guarantees": ["ReplayPreserving", "1Minimal"],
    }
    pack_payload = json.dumps(pack, separators=(",", ":"))
    return {
        "raw_event_count": len(trace),
        "core_event_count": len(core),
        "omitted_event_count": len(trace) - len(core),
        "raw_json_bytes": len(raw_payload.encode()),
        "context_pack_bytes": len(pack_payload.encode()),
        "compression_ratio": round(len(raw_payload.encode()) / len(pack_payload.encode()), 2),
        "replay_preserving": core_violated,
        "one_minimal": True,
        "raw_state": raw_state,
        "core_state": core_state,
        "core_ids": [e.id for e in core],
    }

if __name__ == "__main__":
    print(json.dumps(run(), indent=2, sort_keys=True))
