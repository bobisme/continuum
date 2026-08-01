import Lean
import Continuum

/-!
# T0/T1 axiom manifest generator (ADR-0035)

`ADR-0035` requires every strongest-assurance result to carry machine-captured
axiom output next to its theorem names. This file generates the PR-4A T0/T1
manifest: it walks the RFC 0012 theorem ladder declared in `ladder`
below, collects the axiom dependencies of every listed theorem with the same
`Lean.collectAxioms` the `#print axioms` command uses, and writes two checked-in
artifacts under `lean/artifacts/`:

* `axiom-manifest-t0-t1.json` — the machine-readable manifest;
* `axiom-manifest-t0-t1.txt` — the human-auditable transcript, in the exact
  wording `#print axioms` prints.

Run it from the `lean/` directory (see `scripts/axiom-manifest.sh`):

```sh
lake env lean AxiomManifest.lean
```

The generator fails if a listed theorem does not exist, so renaming a rung
theorem without updating the manifest is a build error rather than a silent gap.

This file is deliberately *not* part of the `Continuum` library target: it is
tooling that imports `Lean`, and nothing in the metatheory may depend on it.
-/

open Lean

namespace Continuum.Manifest

/-- The RFC 0012 theorem ladder for PR-4A, grouped by rung and rung item. The
strings are the RFC's own wording. -/
def ladder : List (String × String × List Name) :=
  [ ("T0", "reachability induction",
      [ `Continuum.TransitionSystem.reachable_satisfies,
        `Continuum.TransitionSystem.reachable_isInductive,
        `Continuum.TransitionSystem.reachable_least,
        `Continuum.TransitionSystem.reachable_iff_exists_path,
        `Continuum.TransitionSystem.isStutterRun_reachable,
        `Continuum.TransitionSystem.isRun_reachable,
        `Continuum.TransitionSystem.isRun_toStutterRun ]),
    ("T0", "inductive invariant preservation",
      [ `Continuum.TransitionSystem.safety_iff_exists_inductiveInvariant,
        `Continuum.TransitionSystem.InductiveInvariant.and,
        `Continuum.TransitionSystem.InductiveInvariant.top,
        `Continuum.TransitionSystem.InductiveInvariant.mono,
        `Continuum.TransitionSystem.InductiveInvariant.holds_on_path,
        `Continuum.TransitionSystem.always_of_inductiveInvariant ]),
    ("T0", "finite closure certificate soundness",
      [ `Continuum.ClosureCertificate.provesSafety,
        `Continuum.ClosureCertificate.isInductiveInvariant,
        `Continuum.ClosureCertificate.exists_iff_safety,
        `Continuum.FiniteSystem.checkClosure_eq_true_iff,
        `Continuum.FiniteSystem.checkClosure_sound,
        `Continuum.FiniteSystem.checkClosure_provesSafety,
        `Continuum.FiniteSystem.checkClosure_eq_false_of_unclosed,
        `Continuum.FiniteSystem.checkClosure_eq_false_of_missing_init ]),
    ("T0", "counterexample path validity",
      [ `Continuum.TransitionSystem.pathFrom_append,
        `Continuum.TransitionSystem.reachable_endpoint,
        `Continuum.TransitionSystem.reachable_of_initial_path,
        `Continuum.TransitionSystem.exists_path_of_reachable,
        `Continuum.FiniteSystem.checkTrace_sound,
        `Continuum.FiniteSystem.checkCounterexample_sound,
        `Continuum.FiniteSystem.no_closure_of_counterexample ]),
    ("T0", "checker trusted base (reflective plumbing)",
      [ `Continuum.Reflect.andLeft,
        `Continuum.Reflect.andRight,
        `Continuum.Reflect.andIntro,
        `Continuum.Reflect.mem_sound,
        `Continuum.Reflect.mem_complete,
        `Continuum.Reflect.all_sound,
        `Continuum.Reflect.all_complete,
        `Continuum.TransitionSystem.endpoint_append,
        `Continuum.FiniteSystem.toTransitionSystem_init,
        `Continuum.FiniteSystem.toTransitionSystem_step ]),
    ("T0", "instantiation: Die Hard finite closure and counterexample",
      [ `Continuum.Examples.DieHard.closure_accepted,
        `Continuum.Examples.DieHard.jugInvariant_of_reachable,
        `Continuum.Examples.DieHard.typeOK_of_reachable,
        `Continuum.Examples.DieHard.closure_without_init_rejected,
        `Continuum.Examples.DieHard.closure_not_closed_rejected,
        `Continuum.Examples.DieHard.closure_unsafe_member_rejected,
        `Continuum.Examples.DieHard.four_gallons_witness,
        `Continuum.Examples.DieHard.four_gallons_reachable,
        `Continuum.Examples.DieHard.skipping_path_rejected ]),
    ("T1", "stuttering simulation preserves reachable safety predicates",
      [ `Continuum.StutteringSimulation.maps_reachable,
        `Continuum.StutteringSimulation.transfer_safety,
        `Continuum.StutteringSimulation.toReachable,
        `Continuum.ReachableSimulation.maps_reachable,
        `Continuum.ReachableSimulation.transfer_safety,
        `Continuum.ReachableSimulation.of_inductiveInvariant ]),
    ("T1", "composition of simulations",
      [ `Continuum.StutteringSimulation.refl,
        `Continuum.StutteringSimulation.compose,
        `Continuum.ReachableSimulation.compose,
        `Continuum.StutteringSimulation.transfers_reachable_property ]),
    ("T1", "observer projection and hidden-event closure",
      [ `Continuum.TransitionSystem.observe_step_visible,
        `Continuum.TransitionSystem.observe_simulation,
        `Continuum.TransitionSystem.observe_reachable_iff,
        `Continuum.TransitionSystem.observe_safety_iff ]),
    ("T1", "history/auxiliary variable erasure",
      [ `Continuum.HistoryAugmentation.erasure_simulation,
        `Continuum.HistoryAugmentation.reachable_lift,
        `Continuum.HistoryAugmentation.erases_safety ]),
    ("T1", "conditions for liveness preservation",
      [ `Continuum.StutteringSimulation.projects_run,
        `Continuum.StutteringSimulation.projects_run_of_visible,
        `Continuum.StutteringSimulation.transfer_eventually,
        `Continuum.StutteringSimulation.transfer_infinitelyOften ]) ]

def quote (s : String) : String :=
  "\"" ++ (s.replace "\\" "\\\\").replace "\"" "\\\"" ++ "\""

/-- One manifest row: the theorem and the axioms its proof term depends on. -/
structure Row where
  rung : String
  item : String
  name : Name
  axioms : List Name

def collectRows : CoreM (Array Row) := do
  let env ← getEnv
  let mut rows := #[]
  for (rung, item, names) in ladder do
    for name in names do
      unless env.contains name do
        throwError "axiom manifest lists unknown declaration {name}"
      let axioms ← collectAxioms name
      rows := rows.push { rung, item, name, axioms := axioms.toList }
  return rows

def renderText (rows : Array Row) : String := Id.run do
  let mut out :=
    "# Continuum T0/T1 axiom manifest (ADR-0035, RFC 0012 theorem ladder)\n" ++
    "# requirement: PR-4A-IMPL-03\n" ++
    "# toolchain: leanprover/lean4:v4.32.1\n" ++
    "# generator: lean/AxiomManifest.lean (Lean.collectAxioms, same source as #print axioms)\n" ++
    "# regenerate: cd lean && sh scripts/axiom-manifest.sh\n"
  let mut currentGroup := ""
  for row in rows do
    let group := row.rung ++ " — " ++ row.item
    if group != currentGroup then
      currentGroup := group
      out := out ++ "\n## " ++ group ++ "\n"
    if row.axioms.isEmpty then
      out := out ++ s!"'{row.name}' does not depend on any axioms\n"
    else
      let names := String.intercalate ", " (row.axioms.map toString)
      out := out ++ s!"'{row.name}' depends on axioms: [{names}]\n"
  return out

def renderJson (rows : Array Row) : String := Id.run do
  let withAxioms := rows.filter (fun row => !row.axioms.isEmpty)
  let mut out :=
    "{\n" ++
    "  " ++ quote "schema" ++ ": " ++ quote "continuum.axiom-manifest/v1" ++ ",\n" ++
    "  " ++ quote "requirement" ++ ": " ++ quote "PR-4A-IMPL-03" ++ ",\n" ++
    "  " ++ quote "ladder" ++ ": " ++ quote "RFC-0012" ++ ",\n" ++
    "  " ++ quote "policy" ++ ": " ++ quote "ADR-0035" ++ ",\n" ++
    "  " ++ quote "leanToolchain" ++ ": " ++ quote "leanprover/lean4:v4.32.1" ++ ",\n" ++
    "  " ++ quote "generator" ++ ": " ++ quote "lean/AxiomManifest.lean" ++ ",\n" ++
    "  " ++ quote "theoremCount" ++ ": " ++ toString rows.size ++ ",\n" ++
    "  " ++ quote "theoremsWithAxioms" ++ ": " ++ toString withAxioms.size ++ ",\n" ++
    "  " ++ quote "theorems" ++ ": [\n"
  let mut first := true
  for row in rows do
    let axioms := String.intercalate ", " (row.axioms.map (fun a => quote (toString a)))
    let sep := if first then "" else ",\n"
    first := false
    out := out ++ sep ++
      "    {" ++ quote "rung" ++ ": " ++ quote row.rung ++ ", " ++
      quote "rungItem" ++ ": " ++ quote row.item ++ ", " ++
      quote "theorem" ++ ": " ++ quote (toString row.name) ++ ", " ++
      quote "axioms" ++ ": [" ++ axioms ++ "]}"
  out := out ++ "\n  ]\n}\n"
  return out

end Continuum.Manifest

open Continuum.Manifest in
#eval show CoreM Unit from do
  let rows ← collectRows
  IO.FS.createDirAll "artifacts"
  IO.FS.writeFile "artifacts/axiom-manifest-t0-t1.txt" (renderText rows)
  IO.FS.writeFile "artifacts/axiom-manifest-t0-t1.json" (renderJson rows)
  let withAxioms := rows.filter (fun row => !row.axioms.isEmpty)
  IO.println s!"axiom manifest written: {rows.size} theorems, {withAxioms.size} with axioms"
  for row in withAxioms do
    IO.println s!"  {row.name}: {row.axioms}"
