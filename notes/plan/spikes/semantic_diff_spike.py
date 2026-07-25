#!/usr/bin/env python3
from __future__ import annotations
from dataclasses import dataclass, replace
import json

@dataclass(frozen=True)
class Intent:
    property_terms: frozenset[str]
    assumptions: frozenset[str]
    nodes: int
    observer_events: frozenset[str]
    faults: frozenset[str]
    assurance: str

ORDER = {"observed":0,"sampled":1,"bounded":2,"validated":3,"proved":4}

def classify(before: Intent, after: Intent) -> list[dict]:
    out=[]
    if before.property_terms == after.property_terms: rel="unchanged"
    elif before.property_terms < after.property_terms: rel="strengthened"
    elif after.property_terms < before.property_terms: rel="weakened"
    else: rel="incomparable"
    out.append({"field":"property","relation":rel,"protected":rel!="unchanged"})

    if before.assumptions == after.assumptions: rel="unchanged"
    elif before.assumptions < after.assumptions: rel="strengthened"
    elif after.assumptions < before.assumptions: rel="weakened"
    else: rel="incomparable"
    out.append({"field":"assumptions","relation":rel,"protected":rel!="unchanged"})

    rel="unchanged" if before.nodes==after.nodes else ("expanded" if after.nodes>before.nodes else "contracted")
    out.append({"field":"bounds.nodes","relation":rel,"protected":rel=="contracted"})

    if before.observer_events == after.observer_events: rel="unchanged"
    elif before.observer_events < after.observer_events: rel="refined"
    elif after.observer_events < before.observer_events: rel="coarsened"
    else: rel="incomparable"
    out.append({"field":"observer","relation":rel,"protected":rel!="unchanged"})

    if before.faults == after.faults: rel="unchanged"
    elif before.faults < after.faults: rel="expanded"
    elif after.faults < before.faults: rel="contracted"
    else: rel="incomparable"
    out.append({"field":"faults","relation":rel,"protected":rel in {"contracted","incomparable"}})

    rel="unchanged" if before.assurance==after.assurance else ("upgraded" if ORDER[after.assurance]>ORDER[before.assurance] else "downgraded")
    out.append({"field":"assurance","relation":rel,"protected":rel=="downgraded"})
    return out


def run() -> dict:
    base=Intent(frozenset({"Ack","Durable"}),frozenset({"SyncImpliesStable"}),5,frozenset({"ReplyPublished","SyncCompleted"}),frozenset({"cancel","crash"}),"validated")
    cases={
        "source_guard_repair": base,
        "property_weakened": replace(base, property_terms=frozenset({"Ack"})),
        "assumption_strengthened": replace(base, assumptions=base.assumptions|{"NoCrashBeforeAck"}),
        "bound_reduced": replace(base, nodes=3),
        "observer_hidden": replace(base, observer_events=frozenset({"ReplyPublished"})),
        "fault_removed": replace(base, faults=frozenset({"cancel"})),
        "assurance_downgraded": replace(base, assurance="sampled"),
    }
    results={k:classify(base,v) for k,v in cases.items()}
    expected={
        "source_guard_repair": ("property","unchanged"),
        "property_weakened": ("property","weakened"),
        "assumption_strengthened": ("assumptions","strengthened"),
        "bound_reduced": ("bounds.nodes","contracted"),
        "observer_hidden": ("observer","coarsened"),
        "fault_removed": ("faults","contracted"),
        "assurance_downgraded": ("assurance","downgraded"),
    }
    checks={}
    for case,(field,relation) in expected.items():
        entry=next(x for x in results[case] if x["field"]==field)
        checks[case]=entry["relation"]==relation and (case=="source_guard_repair" or entry["protected"])
    assert all(checks.values())
    serialized_base = {
        "property_terms": sorted(base.property_terms),
        "assumptions": sorted(base.assumptions),
        "nodes": base.nodes,
        "observer_events": sorted(base.observer_events),
        "faults": sorted(base.faults),
        "assurance": base.assurance,
    }
    return {"base":serialized_base,"cases":results,"checks":checks,"all_detected":all(checks.values())}

if __name__ == '__main__': print(json.dumps(run(),indent=2,sort_keys=True,default=list))
