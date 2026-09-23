//! The output budget: one bound on everything the front end allocates in proportion to
//! its input, charged before the allocation happens.
//!
//! Decision: RFC 0003 ("Predictable elaboration"), INV-016 (source is untrusted), and
//! security review cr-35xovl. A resource
//! bound that is checked after an allocation is not a bound: the allocation it guards
//! has already happened. Every site in this crate that can allocate more than a
//! constant times what it has already been charged for — copying an inlined `let`,
//! substituting a `def`'s arguments, materializing an inferred type, building a
//! canonical sort key, lowering an equivalence that mentions its operands twice —
//! first computes the exact size of what it is about to build, *without building it*,
//! charges that size here, and only then allocates. When the charge would exceed the
//! budget the site returns a typed resource error and allocates nothing.
//!
//! The same holds for time (security review cr-8zf003, round 4): a *work* budget
//! ([`MAX_WORK`] units of fuel) is charged for every unit of work whose count is not
//! already linear in the input — a scope or name-table lookup, a unification step, an
//! occurs-check step, an expression visit during init enumeration, a sort comparison.
//! Where the work can be predicted it is charged before it starts (init enumeration
//! charges candidates × predicate cost and refuses before it enumerates anything);
//! where it cannot, it is charged as it runs and checked at least once per operation,
//! and one operation's own work is bounded by the output budget. Exhausting it is the
//! typed error `WorkLimitExceeded`.
//!
//! The site inventory is in the crate documentation ("Every allocation proportional to
//! input is pre-charged").

/// The most nodes (expression nodes and type nodes together) one elaboration may
/// allocate, and separately the most one lowering may allocate.
pub const MAX_NODES: usize = 1 << 20;

/// The deepest type any pass may hold. Types are recursive trees, and derived `Clone`,
/// `Drop`, and equality recurse over them, so a type deeper than this is refused before
/// it is built.
pub const MAX_TYPE_DEPTH: usize = 128;

/// Bytes of owned text (names, string literals) that count as one node. Text is
/// charged with the node that owns it, so a copied name costs what it allocates.
pub const BYTES_PER_NODE: usize = 32;

/// The node charge for `bytes` of owned text.
pub(crate) const fn text_cost(bytes: usize) -> usize {
    bytes.div_ceil(BYTES_PER_NODE)
}

/// The most units of work (fuel) one elaboration may spend, and separately the most
/// one lowering may spend. One unit is one step of a lookup, unification, traversal,
/// comparison, or evaluation; lookups in ordered tables charge the logarithm of the
/// table size, so the count tracks comparisons up to a constant factor.
pub const MAX_WORK: u64 = 1 << 31;

/// The resource limits of one elaboration or one lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The output budget, in nodes.
    pub nodes: usize,
    /// The work budget, in units of fuel.
    pub work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            nodes: MAX_NODES,
            work: MAX_WORK,
        }
    }
}

/// What one elaboration or one lowering spent, for instrumentation and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    /// Nodes charged to the output budget.
    pub nodes: usize,
    /// Units of fuel charged to the work budget.
    pub work: u64,
}

/// The cost of one lookup of a `len`-byte key in an ordered table of `entries` keys:
/// one comparison per level of the search, each over the key's text.
pub(crate) fn lookup_cost(entries: usize, len: usize) -> u64 {
    let levels = usize::BITS.saturating_sub(entries.leading_zeros()).max(1);
    u64::from(levels).saturating_mul(text_cost(len).max(1) as u64)
}

/// The cost of sorting `n` items whose keys total `bytes` bytes: at most `⌈log₂ n⌉`
/// comparisons per item, each over one key.
pub(crate) fn sort_cost(n: usize, bytes: usize) -> u64 {
    let levels = usize::BITS.saturating_sub(n.leading_zeros()).max(1);
    let per_level = (n as u64).saturating_add(text_cost(bytes) as u64);
    u64::from(levels).saturating_mul(per_level)
}

/// A countdown of work units still allowed.
#[derive(Debug)]
pub(crate) struct Fuel {
    limit: u64,
    left: u64,
}

impl Fuel {
    /// A budget of `limit` units.
    pub(crate) const fn new(limit: u64) -> Self {
        Self { limit, left: limit }
    }

    /// Spend `n` units, or report that they do not fit and spend nothing, so the usage
    /// reported after a refusal is the work actually done.
    pub(crate) fn burn(&mut self, n: u64) -> Result<(), Exhausted> {
        match self.left.checked_sub(n) {
            Some(left) => {
                self.left = left;
                Ok(())
            }
            None => Err(Exhausted),
        }
    }

    /// Units spent so far.
    pub(crate) const fn used(&self) -> u64 {
        self.limit.saturating_sub(self.left)
    }
}

/// A charge that did not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Exhausted;

/// A countdown of nodes still allowed.
#[derive(Debug)]
pub(crate) struct Budget {
    limit: usize,
    left: usize,
}

impl Budget {
    /// A budget of `limit` nodes.
    pub(crate) const fn new(limit: usize) -> Self {
        Self { limit, left: limit }
    }

    /// Nodes charged so far.
    pub(crate) const fn used(&self) -> usize {
        self.limit.saturating_sub(self.left)
    }

    /// Reserve `n` nodes, or report that they do not fit and reserve nothing.
    pub(crate) fn charge(&mut self, n: usize) -> Result<(), Exhausted> {
        match self.left.checked_sub(n) {
            Some(left) => {
                self.left = left;
                Ok(())
            }
            None => Err(Exhausted),
        }
    }
}
