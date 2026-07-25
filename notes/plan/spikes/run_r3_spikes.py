#!/usr/bin/env python3
from __future__ import annotations
import json
from pathlib import Path
import agent_context_spike, semantic_diff_spike, agent_protocol_spike, forge_cegis_spike, incremental_verification_spike, causal_debugger_spike, proof_lens_spike, evidence_graph_spike

def main():
 results={
  "context_pack":agent_context_spike.run(),
  "semantic_diff":semantic_diff_spike.run(),
  "agent_protocol":agent_protocol_spike.run(),
  "forge_cegis":forge_cegis_spike.run(),
  "incremental":incremental_verification_spike.run(),
  "causal_debugger":causal_debugger_spike.run(),
  "proof_lens":proof_lens_spike.run(),
  "evidence_graph":evidence_graph_spike.run(),
 }
 out=Path(__file__).parent/'results/r3-spike-results.json';out.parent.mkdir(exist_ok=True);out.write_text(json.dumps(results,indent=2,sort_keys=True)+"\n")
 try:
  display=out.relative_to(Path.cwd())
 except ValueError:
  display=out
 print(f"wrote {display}; {len(results)} spike groups passed")
if __name__=='__main__':main()
