#!/usr/bin/env python3
from __future__ import annotations
import json

STATES=[{"reserved":r,"synced":s} for r in (False,True) for s in (False,True)]
CANDIDATES={
 "true":lambda x:True,
 "false":lambda x:False,
 "reserved":lambda x:x["reserved"],
 "not_reserved":lambda x:not x["reserved"],
 "not_synced":lambda x:not x["synced"],
 "reserved_and_synced":lambda x:x["reserved"] and x["synced"],
 "synced":lambda x:x["synced"],
}
SIZE={"true":1,"false":1,"reserved":1,"not_reserved":2,"not_synced":2,"synced":1,"reserved_and_synced":3}

def violations(name):
 f=CANDIDATES[name]; out=[]
 for s in STATES:
  ack=f(s)
  if ack and not s["synced"]: out.append({"kind":"safety","state":s})
  if s["reserved"] and s["synced"] and not ack: out.append({"kind":"progress","state":s})
 return out

def run():
 remaining=list(CANDIDATES)
 examples=[]; iterations=[]
 while True:
  remaining.sort(key=lambda n:(SIZE[n],n))
  chosen=remaining[0]
  bad=violations(chosen)
  iterations.append({"chosen":chosen,"remaining":len(remaining),"violation":bad[0] if bad else None})
  if not bad:
   break
  ce=bad[0]; examples.append(ce)
  def agrees(candidate):
   f=CANDIDATES[candidate]; s=ce["state"]
   if ce["kind"]=="safety": return not (f(s) and not s["synced"])
   return f(s)
  remaining=[c for c in remaining if agrees(c)]
  assert remaining
 assert chosen=="synced"
 valid=[n for n in CANDIDATES if not violations(n)]
 assert set(valid)=={"synced","reserved_and_synced"}
 return {"iterations":iterations,"counterexamples":examples,"solution":chosen,"all_valid_candidates":valid,"solution_ast_size":SIZE[chosen],"safety":True,"progress_nonvacuity":True,"finite_grammar_exhausted_for_invalids":True}

if __name__=='__main__': print(json.dumps(run(),indent=2,sort_keys=True))
