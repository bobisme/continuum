#!/usr/bin/env python3
from __future__ import annotations
import hashlib,json

DEPS={
 "parse_model":{"model_file"},
 "parse_source":{"source_file"},
 "elaborate_model":{"parse_model","domain_profile"},
 "extract_program":{"parse_source","domain_profile"},
 "correspondence":{"elaborate_model","extract_program","mapping_file"},
 "property":{"intent_property","observer"},
 "explore_model":{"elaborate_model","property","bounds","faults"},
 "check_refinement":{"correspondence","explore_model"},
 "context_pack":{"check_refinement","source_file","intent_assumptions"},
 "proof_receipt":{"check_refinement","proof_file","lean_epoch"},
}

def cone(changed):
 invalid=set(changed); progress=True
 while progress:
  progress=False
  for q,deps in DEPS.items():
   if q not in invalid and deps & invalid:
    invalid.add(q); progress=True
 return sorted(x for x in invalid if x in DEPS)

def sig(name,inputs,cache):
 vals=[]
 for d in sorted(DEPS[name]): vals.append(cache.get(d,inputs.get(d,d)))
 return hashlib.sha256((name+json.dumps(vals,sort_keys=True)).encode()).hexdigest()[:12]

def build(inputs):
 cache={}
 for q in DEPS: cache[q]=sig(q,inputs,cache)
 return cache

def incremental(before_inputs,after_inputs):
 before=build(before_inputs); changed={k for k in set(before_inputs)|set(after_inputs) if before_inputs.get(k)!=after_inputs.get(k)}
 inv=cone(changed); cache=dict(before)
 for q in DEPS:
  if q in inv: cache[q]=sig(q,after_inputs,cache)
 clean=build(after_inputs)
 return {"changed":sorted(changed),"invalidated":inv,"parity":cache==clean,"reused":sorted(set(DEPS)-set(inv))}

def run():
 base={"model_file":"m1","source_file":"s1","domain_profile":"d1","mapping_file":"map1","intent_property":"p1","observer":"o1","bounds":"b1","faults":"f1","intent_assumptions":"a1","proof_file":"l1","lean_epoch":"4.32.1"}
 cases={
  "property_edit":{**base,"intent_property":"p2"},
  "source_edit":{**base,"source_file":"s2"},
  "model_edit":{**base,"model_file":"m2"},
  "proof_only":{**base,"proof_file":"l2"},
  "domain_profile":{**base,"domain_profile":"d2"},
 }
 out={k:incremental(base,v) for k,v in cases.items()}
 assert all(x["parity"] for x in out.values())
 assert "parse_source" not in out["property_edit"]["invalidated"]
 assert out["proof_only"]["invalidated"]==["proof_receipt"]
 assert "context_pack" in out["source_edit"]["invalidated"]
 return {"query_count":len(DEPS),"cases":out,"all_clean_parity":True}

if __name__=='__main__': print(json.dumps(run(),indent=2,sort_keys=True))
