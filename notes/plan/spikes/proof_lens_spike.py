#!/usr/bin/env python3
from __future__ import annotations
import json

def get_candidates(concrete):
 return [
  {"name":"stable_is_durable","abstract":{"durable":concrete["stable"],"acked":concrete["published"]},"evidence_needed":"storage profile says stable is authoritative"},
  {"name":"quorum_is_durable","abstract":{"durable":concrete["quorum_acked"],"acked":concrete["published"]},"evidence_needed":"quorum acknowledgements imply recovery durability"},
 ]

def put_durable(concrete,target=True):
 candidates=[]
 if concrete["stable"]!=target:
  c=dict(concrete);c["stable"]=target;candidates.append({"edit":"set stable","concrete":c,"obligation":"storage operation exists and preserves refinement"})
 if concrete["quorum_acked"]!=target:
  c=dict(concrete);c["quorum_acked"]=target;candidates.append({"edit":"obtain quorum ack","concrete":c,"obligation":"quorum profile implies durable"})
 return candidates

def run():
 c={"stable":False,"quorum_acked":False,"published":False}
 projections=get_candidates(c); edits=put_durable(c,True)
 conflict={"kind":"AmbiguousCorrespondence","abstract_field":"durable","projection_alternatives":[x["name"] for x in projections],"edit_alternatives":[x["edit"] for x in edits],"required_obligation":"select authoritative durability semantics in Intent/domain profile"}
 assert len(projections)==2 and len(edits)==2
 # A field with direct provenance is unambiguous.
 ack_edit={**c,"published":True}; assert get_candidates(ack_edit)[0]["abstract"]["acked"]
 return {"ambiguous_durable":conflict,"silent_choice_made":False,"unambiguous_ack_roundtrip":True,"candidate_count":len(edits)}

if __name__=='__main__':print(json.dumps(run(),indent=2,sort_keys=True))
