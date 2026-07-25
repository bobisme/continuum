#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json

class StaleSnapshot(Exception): pass

def digest(obj) -> str:
    b=json.dumps(obj,sort_keys=True,separators=(",",":")).encode()
    return hashlib.sha256(b).hexdigest()[:20]

class Workbench:
    def __init__(self):
        self.snapshots={}; self.tasks={}; self.idempotency={}
    def create_snapshot(self, files, intent):
        manifest={"files":dict(sorted(files.items())),"intent":intent}
        h="ws_"+digest(manifest); self.snapshots[h]=manifest; return h
    def start(self, snapshot, intent, operation, idempotency_key):
        req={"snapshot":snapshot,"intent":intent,"operation":operation}
        rd=digest(req)
        if idempotency_key in self.idempotency:
            old_rd,task=self.idempotency[idempotency_key]
            if old_rd != rd: raise ValueError("idempotency key reused with different request")
            return task
        task="task_"+rd; self.tasks[task]={"request":req,"progress":0,"status":"running"}
        self.idempotency[idempotency_key]=(rd,task); return task
    def suspend(self, task, progress):
        t=self.tasks[task]; t["progress"]=progress;t["status"]="suspended"
        return "cont_"+digest({"task":task,"request":t["request"],"progress":progress})
    def resume(self, continuation, snapshot):
        for task,t in self.tasks.items():
            expected="cont_"+digest({"task":task,"request":t["request"],"progress":t["progress"]})
            if expected==continuation:
                if t["request"]["snapshot"]!=snapshot: raise StaleSnapshot(snapshot)
                t["status"]="running"; return task
        raise KeyError("unknown continuation")

def run():
    wb=Workbench(); intent="in_demo"
    ws1=wb.create_snapshot({"src/lib.rs":"v1"},intent)
    task1=wb.start(ws1,intent,"verify:AckImpliesDurable","idem-1")
    task2=wb.start(ws1,intent,"verify:AckImpliesDurable","idem-1")
    assert task1==task2
    cont=wb.suspend(task1,37)
    assert wb.resume(cont,ws1)==task1
    ws2=wb.create_snapshot({"src/lib.rs":"v2"},intent)
    stale=False
    try: wb.resume(cont,ws2)
    except StaleSnapshot: stale=True
    assert stale
    mismatch=False
    try: wb.start(ws2,intent,"verify:Other","idem-1")
    except ValueError: mismatch=True
    assert mismatch
    return {"snapshot_1":ws1,"snapshot_2":ws2,"task":task1,"idempotent":task1==task2,"continuation":cont,"resume_valid":True,"stale_snapshot_rejected":stale,"idempotency_mismatch_rejected":mismatch}

if __name__=='__main__': print(json.dumps(run(),indent=2,sort_keys=True))
