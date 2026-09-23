//! `continuum-cml-elab` — CML elaboration to the normalized semantic AST (PR 15a,
//! docs/11 §14).
//!
//! # Responsibility
//!
//! Elaborates parsed CML into the normalized semantic AST and, from there, into the
//! same semantic model identity a programmatic model produces.
//!
//! The normalized AST is frozen before surface syntax, so the deterministic formatter
//! and the migration tool (PR 15b) are defined over this crate's output, not over the
//! token stream.
//!
//! # What this crate delivers
//!
//! Decision: RFC 0003 (the model language) and ADR-0025 (the Finite fragment).
//!
//! - [`elaborate`] resolves names, infers and checks types, and produces a
//!   [`NormModel`] ([`norm`]) — or exactly one typed, source-located [`ElabError`]. A
//!   well-formed model that uses semantics outside the Finite core fragment is
//!   [`ElabErrorKind::Unsupported`], never approximated.
//! - [`NormModel::identity`] is content-addressed from the normalized AST, not from the
//!   source text: header style, `=` versus `==`, declaration order, `let` and `def`
//!   names, bound-variable names, and set-literal order do not change it.
//! - [`lower()`] turns a normalized model into a [`continuum_model_core::Model`] through
//!   [`continuum_model_core::ModelBuilder`] — the programmatic API itself. There is one
//!   model type, so "the same model" is [`continuum_model_core::Model::identity`]
//!   equality between an elaborated and a hand-built model. What the programmatic model
//!   cannot carry is a typed [`lower::Unlowerable`] reason.
//! - Every output is deterministic: a pure function of the parsed tree, with no
//!   hash-ordered collection (INV-005).
//!
//! The decisions the parser left open (binders without a domain, the two header
//! styles, `=` and `==`, `def`) are recorded in the [`elab`] and [`norm`] module
//! documentation.
//!
//! # Every allocation proportional to input is pre-charged
//!
//! Source is untrusted (INV-016). Security reviews cr-35xovl and cr-8zf003 found
//! allocations checked only after they happened, trees deep enough to overflow the
//! stack, and loops whose work was quadratic in the input. The rule since then has
//! three budgets, all in [`budget`], each one per elaboration and again per lowering:
//!
//! - **size**: an output budget ([`budget::MAX_NODES`] nodes). Every site that can
//!   allocate more than a constant times what was already charged first computes the
//!   exact size of what it will build, *without building it*, charges it, and then
//!   allocates. A node costs one, plus the text it owns (`budget::text_cost`), plus the
//!   structural size of its type and its binders' types.
//! - **depth**: every tree kind is refused *before* it would pass its depth bound, so
//!   no pass recurses, and no `Drop` recurses, deeper than the bound.
//! - **work**: a fuel budget ([`budget::MAX_WORK`] units). A unit is one step of a
//!   lookup, unification, traversal, comparison, or evaluation; a lookup in an ordered
//!   table charges the logarithm of the table size. Predictable work is charged before
//!   it starts; other work is charged as it runs and checked at least once per
//!   operation, and a single operation's work is bounded by the output budget.
//!
//! Over a budget is a typed error: [`ElabErrorKind::TooLarge`] and
//! [`ElabErrorKind::WorkLimitExceeded`] in elaboration; [`Unlowerable::OutputTooLarge`],
//! [`Unlowerable::ExpressionTooDeep`], and [`Unlowerable::WorkLimitExceeded`] in
//! lowering. [`elaborate_with`] and [`lower_with`] take explicit [`Limits`] and report
//! the [`Usage`] spent, which is how `tests/resource_bounds.rs` counts operations
//! instead of timing them.
//!
//! The inventory below lists every loop, recursion, and amplifying allocation in `elab`,
//! `types`, `lower`, and `norm` whose cost is not a constant per source token.
//!
//! | Site | Size | Depth | Work |
//! |---|---|---|---|
//! | `elab::expr` and its per-form helpers (one call per syntax node) | each node charged in `node` before construction: one, its text, its type | syntax nesting ≤ the parser's `MAX_NESTING`; node types ≤ [`budget::MAX_TYPE_DEPTH`], checked before storage | constant per node, plus the charged lookups below |
//! | `Scope` (params, `let`s, binders): push, lookup, freshness | entries are source names | flat | an ordered index, not a scan: `lookup_cost` per lookup and per freshness check (`check_fresh`, `name`) |
//! | global name table (`declare`, `look`, `state_type`, type names, calls, `step`, choices, fairness) | declarations | flat | `lookup_cost` per lookup |
//! | record literals and record types: field uniqueness | fields | flat | an ordered set, not a scan; `sort_cost` charged for the check and the field sort |
//! | copying a `let` value at each use | measured once at the `let` (work: its size); each use charged before the clone | ≤ [`elab::MAX_INLINE_DEPTH`], checked before the clone | the copy's size |
//! | substituting a non-recursive `def` call | `measure_with` computes the substituted size (a parameter used `k` times counts its argument `k` times); charged before `substitute` | same computation; ≤ [`elab::MAX_INLINE_DEPTH`], checked before `substitute` | argument measures, plus the body twice with one parameter-table lookup per node (a map, not a scan) |
//! | recursive def termination check (`termination`, `decreases`, `first_recur`) | none | iterative (explicit stacks); `const_int` and `measure_test` recurse over one constant subtree, within the tree's checked depth | predictable, charged before each pass: two units per body node for the scan, three per body node per candidate measure parameter |
//! | recursive call: the measure constant and the chain bound (`unfold`) | none | the chain `v / step` ≤ [`elab::MAX_RECURSION_DEPTH`], checked from the constant before anything is planned | the constant's nodes; each argument's measure |
//! | recursive call: the unfolding plan (`plan_tree`) | computes the exact size of the unfolding, without building it: every emitted node with its type, every substituted parameter at its argument's size, and every argument of every recursive call (built once, used or not); refused as soon as it passes what the budget has left, then charged in one `precharge_copy` | iterative (explicit stack); stops at output depth [`elab::MAX_INLINE_DEPTH`] and at visit nesting [`elab::MAX_UNFOLD_NESTING`]; an argument is planned by one nested call, which meets no recursive call | two units per visited node (the build visits it again) plus each measure test's constant, charged as it runs, checked per node |
//! | recursive call: the build (`Unfolder`) | what the plan charged | recursion ≤ [`elab::MAX_UNFOLD_NESTING`] (the plan's visit nesting), with small `#[inline(never)]` frames | charged by the plan |
//! | recursive call: fresh binder numbers per unfolding (`BuildFrame::rename`) | a transient map, one entry per binder in scope of one body | flat | one ordered-table step per binder and per bound variable, within the plan's two units per node |
//! | alias and def dependency orders (`dependency_order`) | edge lists from the source | explicit stack, no recursion | `order_cost`: one table lookup per edge, charged before the search |
//! | alias resolution (`alias`, `alias_visiting`) | see the next row | dependencies are resolved first, so `alias_visiting` holds at most the one alias being resolved | constant per alias |
//! | resolving a type expression, cloning an alias | computed from the syntax and cached alias measures; charged before building; the alias cache's copy charged too | ≤ [`budget::MAX_TYPE_DEPTH`], checked before building | twice the type's size |
//! | `action`: postconditions (`split_clauses`, `update`, `primed_names`, `has_primed`) | the `x' == e` node of a primed `next`, charged in `node` | iterative scans | one unit per node of each clause, charged before its scan |
//! | `action`: resolving primed reads of updated or kept variables (`resolve_primes`) | each copied update measured and charged before the clone; a kept variable's `State` node charged in `node` | the clause's depth; a copy is refused before it would pass [`elab::MAX_TREE_DEPTH`] | one unit per node, plus the copy's size |
//! | `config::RunConfig::parse` (not budgeted: it runs before a lowering exists) | the input is at most [`config::MAX_CONFIG_BYTES`]; a sort at most [`config::MAX_SORT_ELEMENTS`] elements; `bounds` (`read_bounds`) is at most two ranges of two integers, each range at most [`config::MAX_BOUND_VALUES`] values, its value count computed in `i128` | the strict JSON reader's depth bound (64); `read_value` recurses within it | linear in the bounded input, plus a sort of each `set`/`map` by canonical bytes (`n log n` comparisons; each level re-encodes its members, so at most depth × input bytes) |
//! | `lower::lower_configured`: the bindings (`bindings`, `Tables`, `typecheck`) | the declared sort and constant indexes, every sort element and enumeration variant (a node and its text), every value node with its text by bytes (computed once while reading and stored: `RunConfig::constant_size`, a charged lookup), and every stored binding with its name, charged as output before it is built | `typecheck` recurses within the configuration's JSON depth bound | each name table's build (a sort over its names' bytes) charged before it is built; each lookup (a name→index table: logarithmic in the table, linear in the name) charged before it is made; one unit per value node; no scan of an element list (cr-1sxoia) |
//! | `lower::lower_configured`: identities | the model identity's peak allocation (`Model::identity_alloc_bound`, allocation-free: the output buffer, the one reused outcome scratch buffer, and its range list, each allocated once at that capacity) charged as output and work *before* `Model::identity` runs, and the encoding is never longer (debug-asserted); the configuration identity is shared (`Arc`), never copied; the run identity holds the two by value | the walks recurse as `Model::identity` does (≤ `MAX_EXPR_DEPTH`) | five walks (the bound's two and the identity's three; each at most the charged nodes plus the initial-state values, variables, actions, and predicates) charged before the first runs; then the bound |
//! | `lower`: a sort-typed expression node or state variable | none | flat | one lookup in the instantiated sorts, charged first |
//! | `lower`: an integer state variable's domain under a configuration bound (`domain_of`, bn-15zfa) | none: two integers | flat | constant per refinement clause (the clause visits already charged); the bound's domain enters the init enumeration below, whose cardinality check and `init_work` charge come first, so a wide bound is refused before any candidate |
//! | `lower`: typing a configuration integer against its bound (`typecheck`) | none | within `typecheck`'s recursion | constant per value node, inside the unit already charged per node |
//! | `lower`: model-core's limits (`preflight`) | none; refuses before init enumeration and before any action: variables > `MAX_VARIABLES`, total expanded actions (parameter instances × relational candidates, summed) > `MAX_ACTIONS`, a declared or generated name > `MAX_IDENT_BYTES` (the widest instance name `A(p=u,…)` computed exactly from each universe's longest value text, bn-10j7z), one action's candidates > [`crate::lower::MAX_RELATIONAL_CANDIDATES`] | flat | one table lookup per action entry, per parameter universe, and per value-text table, and one unit per name, charged as it runs |
//! | `lower`: enumeration tables (`enum_tables`, bn-10j7z) | every variant (a node and its text, twice: index and ordered list) and each table's name, charged before the table is built | flat | `sort_cost` over the variant names and a lookup per table, charged first |
//! | `lower`: the state-domain table for intervals (`domains`) | an entry and its name per variable, charged before the table is built | flat | `sort_cost` over the names |
//! | `lower::lower_configured`: sort element names and constant sets (`bindings`, bn-10j7z) | each copied element name (a node and its text) and each set's codes (one node per member), charged before the copy | flat | one unit per element and a lookup per sort; per set member a charged element or variant lookup, and `sort_cost` for the codes, charged before the pass |
//! | `lower`: planned sites (`top_bool`, `lower_action`, `begin_site`, bn-10j7z) | the plan's size (exact without a quantifier, an upper bound with one) charged in one step before the build, which draws from it; an over-estimate stays charged; a build past its plan is a defect caught by a debug assertion and still charged | the plan's depth checked before the build | the plan's predicted work checked against the work left before the build; the plan itself spends one unit per step it predicts; the build spends as it runs, before each step |
//! | `lower`: action schemas (`param_space`, `build_action`, bn-10j7z) | instances ≤ `MAX_ACTIONS` (preflight); instances × (planned guard, postconditions, updates, and name text) charged before the first instance; a relational action's candidates checked in aggregate (instances × `successor_work`) against both budgets before the first instance | each instance's guard and updates at the planned depth | instances × the planned work checked before the first; per instance, two lookups per parameter (scope and name text) |
//! | `lower`: quantifier expansion (`quantifier`, `instance`, `candidates`, bn-10j7z) | planned: fan-out × the item's measure (guard, body) plus `n − 1` combining nodes, nested fan-outs multiplied, saturating; built: each instance charged from the plan, and the fan-out re-checked against what is left before the loop | the plan's `balanced_measure`: item depth + `⌈log₂ n⌉`, checked in the plan; recursion follows the source tree (`shallow`) and at most [`crate::lower::MAX_BINDER_NESTING`] binders in scope, checked before each binder is entered (one `Quant` may list any number) | planned once per quantifier with the item's predicted work multiplied by the fan-out; built: two scope steps per candidate plus the item; a static set literal's codes sorted, the sort spent before it runs in the plan as in the build (`effort`); a constant set shared by reference (`Rc`), never copied; the plan's predicted work checked against the work left at every step (`predicted_fits`) |
//! | `lower`: intervals of domain bounds and members (`interval`), and the plan's own intervals (`Plan::I`), which the guarded-overflow check reads at every integer node planned inside a guarded instance (`int_expr`, cr-3aqchd) | none: `i128` pairs | the source tree's depth (`shallow`) | one unit per node, plus a charged lookup per constant, parameter, binder, variant, or state variable; the plan's intervals are computed with the nodes it already plans, plus a spent lookup per state variable read inside a guarded instance |
//! | `lower`: membership in a set literal or constant set (`membership`, bn-10j7z) | `x` lowered once and each further use a charged copy; one comparison node per member; `n − 1` combining nodes | a balanced `||` tree | a constant set shared by reference, never copied; one node per member; `n` for the combining |
//! | `lower`: composite state layouts (`collections::slots_of`, `laid_var`, bn-23hzh) | the slot count from the types, saturating, checked against `MAX_VARIABLES` before any slot; each slot record and its name (twice: the builder's and the table's) charged before the copy, the name refused past `MAX_IDENT_BYTES` before it is built; the table's copy of the type charged by its size | recursion follows the type, its depth checked against [`budget::MAX_TYPE_DEPTH`] first; at most `MAX_VARIABLES` slots | a unit per value-text node, spent as it is visited; each type computation charged `4 × size × (depth + 128)` first (`collections::type_cost`) |
//! | `lower`: canonicity constraints and definedness predicates (`collections::canonicity`, `definedness_name`, bn-23hzh) | every node charged as built; the predicate record and its name | balanced trees, depth checked before any node | one unit per node; the name's bytes |
//! | `lower`: layouts of expressions (`collections::lay_at` and its arms, bn-23hzh) | planned as an aggregate (width, largest slot, hull), never a list; the width checked against the output left before a vector of it exists; a slot read charged its variable's longest slot name; scratch output (static values lowered to be evaluated, slots selected away) counted into the plan and multiplied at fan-outs | the source tree's depth (`shallow`), each arm its own `#[inline(never)]` frame | per slot, the plan predicts exactly the build's steps (`LayOps`); a type computation per slot charged first |
//! | `lower`: static keys and values (`collections::static_code`, `static_index`, bn-23hzh) | the lowered value is scratch, charged; a composite's codes (one per slot, already charged) | the value's checked depth | its size, to evaluate it with no state |
//! | `lower`: comprehensions (`collections::comprehension`, bn-23hzh) | planned like a quantifier, fan-out multiplied, entries overlaid on the width | at most [`crate::lower::MAX_BINDER_NESTING`] binders in scope, checked before each | two scope steps per candidate; the collected entries' sort charged by the leaves visited (the plan's an upper bound) |
//! | `lower`: definedness items (`collections::LayOps::f_push`, bn-23hzh) | one item per map read, planned as an aggregate and multiplied at fan-outs; a guarded instance's items folded into `guard => items` over a charged copy of the guard | balanced folds, depth checked in the plan | the fold's `n` units and the copies' sizes |
//! | `lower`: planned sites (`begin_site`, `end_site`, bn-23hzh) | as below | as below | a site spends exactly its prediction: the build as it runs, the rest when the site closes (debug-asserted never to pass it), so a lowering replays under the limits it reported |
//! | `lower`: composite constants (`composite_binding`, `atom_of`, bn-23hzh) | a set's member indices, one node each, charged before they are stored | the configuration's JSON depth bound | one unit per value node and per lookup, spent first; the indices' sort spent first |
//! | `lower`: relational actions (`relational_action`) | candidates ≤ [`crate::lower::MAX_RELATIONAL_CANDIDATES`]; `lower::successor_work` (candidates × (template nodes + name text + 1)) charged as output before the first candidate is built | a candidate is its template with constants substituted: the template's checked depth (`subst_bool`, `subst_int` recurse ≤ `MAX_EXPR_DEPTH`) | the same product checked against the work left before the first candidate; then one unit per node copied and per placeholder resolved (by slot number, constant time), charged per candidate |
//! | `action`: the post-state list | one entry per state variable per action, charged as output with its text | flat | one table lookup per statement and per state variable |
//! | clause lists (`require`, init, invariant): `conjuncts` | flat vectors | flat | linear |
//! | `mentions_prime`, `primed_update`, `expr_calls`, `type_refs` | none | the parser's nesting bound | linear: each syntax node visited once |
//! | `finish` (one call per node) | inferred types: `Unifier::measure_resolved` (iterative, memoized over the solution DAG) charged before `resolve` | resolved depth ≤ [`budget::MAX_TYPE_DEPTH`], checked before `resolve` | one unit per node, plus the unifier's counted steps |
//! | set and map literal sort keys (`sort_keys`, `norm::keys_in_scope`) | each element's measure charged before its key text is built | flat | key rendering (its size) plus `sort_cost` for the sort; the binder scope is borrowed with its index (`norm::BinderScope`), never copied or rebuilt |
//! | the finishing binder scope (`BinderScope`) | binder numbers | flat | `lookup_cost` per binder entered |
//! | declaration lists sorted by name (state, actions, choices, invariants, fairness, behaviors, …) | none | flat | `sort_cost` before each sort |
//! | `types::Unifier::unify` | a stored solution's structural size charged before storage | no recursion; solutions ≤ [`budget::MAX_TYPE_DEPTH`] deep | a work list; each pair counts the size of its two heads; union-find merging and path compression (`compress`) settle shared pairs and chains once; steps collected as fuel after every unification and every node |
//! | `types::Unifier::occurs`, `last_var`, `head` | none | iterative; `occurs` stops at [`budget::MAX_TYPE_DEPTH`] | one step per node or chain link, counted |
//! | `types::Unifier::describe` (diagnostics) | a few hundred bytes | a few levels | bounded |
//! | `head_type` (method, index, field typing) | a transient copy of one stored, charged structure | that structure's checked depth | its size |
//! | `lower`: every lowered node (`node`, generic over `Plan` and `Build`) | charged before construction (building only), with variable-name text | ≤ `continuum_model_core`'s `MAX_EXPR_DEPTH` (the depth the builder and evaluator admit, counted their way), checked before construction | one unit, plus a scan of the declared variables when the node names one (the builder's check does that scan) |
//! | `lower`: `<=>`, Boolean `==`/`!=`, Boolean `if`, `in a..b` with non-constant bounds (`copy`) | the second copy of an operand charged before the clone | a copy keeps its checked depth; the node over it is checked | the copy's size |
//! | `lower`: clause lists (`conjoin`) | the `&&` nodes charged before any is built; an empty list is one charged `true` node | a balanced tree, `⌈log₂ n⌉` levels over the deepest clause, checked before any node is built | one unit per clause |
//! | `lower`: `shallow` (pre-recursion depth check) | none | iterative | the clause's size |
//! | `lower`: refinements (`domain_of`, `constant`) | none | normalized-tree depth | the refinement's size, twice |
//! | `lower`: behaviors (`standard_behavior`) | none | flat | the covering choices computed once (linear in the choices); one lookup per behavior |
//! | `lower`: init enumeration | bindings counted before each state is added ([`crate::lower::MAX_INIT_BINDINGS`]) | flat vectors | `lower::init_work` (candidates × (predicate size × (variables + 1) + 2 × variables + 1)) charged *before* enumerating; candidates ≤ [`crate::lower::MAX_INIT_ENUMERATION`]; per accepted state, its copy, placement, and sort in the builder charged before the copy |
//! | `lower`: `ModelBuilder::build` | what was charged above | bounded above | `sort_cost` for the variable, action, and predicate sorts; its validation scans charged per variable-naming node |
//! | `norm`: dump, identity, `scoped_key` | output text proportional to a tree whose every node, text, and type was charged | trees bounded above | linear, with bound-variable depths from `BinderScope`'s index; these are not called during elaboration except through `sort_keys` |
//!
//! Everything not listed is a constant amount of work per source token or per charged
//! node: allocations linear in the source text (declaration tables, names copied once
//! from the parse tree), bounded by the parser's `MAX_SOURCE_BYTES`. Recursion depths are
//! in the [`elab`] module documentation. No error path drops a deep structure: every
//! tree a pass builds is refused *before* it would exceed its depth bound, so derived
//! `Drop`, `Clone`, and equality recurse only over bounded depth. The bounds apply to
//! trees this crate builds; a [`NormModel`] assembled by hand is outside them. Parsing
//! itself belongs to `continuum-cml-syntax`.
//!
//! # Dependency-boundary contract
//!
//! - Depends on `continuum-cml-syntax` — the one workspace edge PR 15a states outright
//!   ("parser and elaborator") — and on `continuum-model-core`, the model the
//!   elaborator lowers to.
//! - A front end, not an authority: it may not own semantic state, publish evidence, or
//!   import engines, adapters, or Forge. The reference engine appears only as a
//!   dev-dependency, for the differential test.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

#![forbid(unsafe_code)]

pub mod budget;
pub mod config;
pub mod elab;
pub mod error;
pub mod lower;
pub mod norm;
pub mod types;

pub use budget::{Limits, Usage};
pub use elab::{elaborate, elaborate_with};
pub use error::{ElabError, ElabErrorKind, Unsupported};
pub use lower::{LowerError, LowerErrorKind, Unlowerable, lower, lower_with};
pub use norm::{NormIdentity, NormModel};
pub use types::Type;

/// Parse and elaborate a `.ctm` source.
///
/// # Errors
///
/// A parse error (as [`ElabErrorKind::Parse`]) or an elaboration error.
pub fn elaborate_source(src: &str) -> Result<NormModel, ElabError> {
    elaborate_source_with(src, Limits::default()).0
}

/// [`elaborate_source`] under explicit resource limits, reporting what elaboration
/// spent. A parse error reports zero usage.
pub fn elaborate_source_with(src: &str, limits: Limits) -> (Result<NormModel, ElabError>, Usage) {
    match continuum_cml_syntax::parse(src) {
        Ok(file) => elaborate_with(&file, limits),
        Err(e) => (
            Err(ElabError::new(ElabErrorKind::Parse(e.clone()), e.span)),
            Usage::default(),
        ),
    }
}
