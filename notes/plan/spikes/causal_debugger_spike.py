#!/usr/bin/env python3
from __future__ import annotations
import json

def enabled(state):
 out=[]
 if state["submitted"] and not state["synced"] and not state["cancelled"]: out.append("SyncCompleted")
 if state["reserved"] and not state["cancelled"] and not state["acked"]: out.append("CancelRequested")
 if state["cancelled"] and state["reserved"] and not state["acked"]: out.append("FinalizerPublishes")
 if state["synced"] and state["reserved"] and not state["acked"]: out.append("NormalPublishes")
 return out

def step(state,event):
 s=dict(state)
 if event=="SyncCompleted":s["synced"]=True
 elif event=="CancelRequested":s["cancelled"]=True
 elif event in {"FinalizerPublishes","NormalPublishes"}:s["acked"]=True
 else:raise ValueError(event)
 return s

def violation(s):return s["acked"] and not s["synced"]

def run():
 root={"submitted":True,"reserved":True,"synced":False,"cancelled":False,"acked":False}
 frontier=enabled(root); assert frontier==["SyncCompleted","CancelRequested"]
 safe1=step(root,"SyncCompleted");safe2=step(safe1,"NormalPublishes")
 fail1=step(root,"CancelRequested");fail2=step(fail1,"FinalizerPublishes")
 assert not violation(safe2) and violation(fail2)
 return {
  "root":root,"enabled_frontier":frontier,
  "why_enabled":{"SyncCompleted":["submitted","not synced","not cancelled"],"CancelRequested":["reply reserved","not cancelled","not acknowledged"]},
  "safe_branch":{"events":["SyncCompleted","NormalPublishes"],"final":safe2,"violation":False},
  "failing_branch":{"events":["CancelRequested","FinalizerPublishes"],"final":fail2,"violation":True},
  "first_conflict":{"safe":"SyncCompleted","failing":"CancelRequested"},
  "abstract_difference":{"safe":"acked && durable","failing":"acked && !durable"},
  "reverse_causal_choices_at_failure":["FinalizerPublishes"],
 }

if __name__=='__main__':print(json.dumps(run(),indent=2,sort_keys=True))
