//! The types of the Finite core fragment, and the unifier that infers them.
//!
//! Decision: RFC 0003 "Type system" and docs/11 §3 ("Type universe"). The fragment's
//! types are `Bool`, `Int`, `Nat`, `String`, uninterpreted sorts, enumerations, tuples,
//! records, `Option`, `Set`, `Map`, `Seq`, and functions.
//!
//! # `Nat` and `Int`
//!
//! `Nat` is `Int` restricted to non-negative values. In expressions both are integers
//! and are compatible with each other: `small - (next_big - big)` is well-typed over two
//! `Nat` variables, and whether the result stays non-negative is a state-domain fact,
//! checked where the value lands (PO-MOD-003), never a typing fact. A declared type keeps
//! its spelling, because the lowering reads `Nat` as the lower bound `0`. An inferred type
//! that meets both `Int` and `Nat` uses is `Nat`, whatever the order of the uses.
//!
//! # Inference
//!
//! A binder written without a domain, `{}`, `[]`, and `None` have types the source does
//! not spell. The elaborator gives each a type variable and solves the variables by
//! first-order unification over the uses. A variable left unsolved at the end of a
//! declaration is an error: the elaborator never picks a type.

use std::fmt;

/// A type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    /// `Bool`.
    Bool,
    /// `Int`.
    Int,
    /// `Nat`.
    Nat,
    /// `String`.
    Str,
    /// An uninterpreted sort.
    Sort(String),
    /// An enumeration.
    Enum(String),
    /// `Option[T]`.
    Option(Box<Type>),
    /// `Set[T]`.
    Set(Box<Type>),
    /// `Map[K, V]`.
    Map(Box<Type>, Box<Type>),
    /// `Seq[T]`.
    Seq(Box<Type>),
    /// `(T, U, …)`.
    Tuple(Vec<Type>),
    /// `{f: T, …}`, fields sorted by name.
    Record(Vec<(String, Type)>),
    /// `T -> U`.
    Function(Box<Type>, Box<Type>),
    /// An inference variable. Never present in an elaborated model.
    Var(u32),
}

impl Type {
    /// Whether the type is `Int` or `Nat`.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Int | Type::Nat)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Bool => f.write_str("Bool"),
            Type::Int => f.write_str("Int"),
            Type::Nat => f.write_str("Nat"),
            Type::Str => f.write_str("String"),
            Type::Sort(name) | Type::Enum(name) => f.write_str(name),
            Type::Option(t) => write!(f, "Option[{t}]"),
            Type::Set(t) => write!(f, "Set[{t}]"),
            Type::Map(k, v) => write!(f, "Map[{k}, {v}]"),
            Type::Seq(t) => write!(f, "Seq[{t}]"),
            Type::Tuple(items) => {
                f.write_str("(")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str(")")
            }
            Type::Record(fields) => {
                f.write_str("{")?;
                for (index, (name, ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{name}: {ty}")?;
                }
                f.write_str("}")
            }
            Type::Function(a, b) => write!(f, "({a} -> {b})"),
            Type::Var(n) => write!(f, "?{n}"),
        }
    }
}

/// The structural size and depth of a type, without following variables, computed
/// without recursion. Size counts one per node plus the text cost of the names the
/// type owns (sorts, enumerations, record fields).
pub(crate) fn type_measure(t: &Type) -> (usize, usize) {
    let mut size = 0_usize;
    let mut deepest = 0_usize;
    let mut stack: Vec<(&Type, usize)> = vec![(t, 1)];
    while let Some((node, depth)) = stack.pop() {
        size = size.saturating_add(1);
        deepest = deepest.max(depth);
        let below = depth.saturating_add(1);
        match node {
            Type::Sort(name) | Type::Enum(name) => {
                size = size.saturating_add(crate::budget::text_cost(name.len()));
            }
            Type::Record(fields) => {
                for (name, t) in fields {
                    size = size.saturating_add(crate::budget::text_cost(name.len()));
                    stack.push((t, below));
                }
            }
            Type::Option(a) | Type::Set(a) | Type::Seq(a) => stack.push((a, below)),
            Type::Map(a, b) | Type::Function(a, b) => {
                stack.push((a, below));
                stack.push((b, below));
            }
            Type::Tuple(items) => stack.extend(items.iter().map(|i| (i, below))),
            _ => {}
        }
    }
    (size, deepest)
}

/// Why a unification failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifyError {
    /// The types differ, or equating them would make a type contain itself.
    Mismatch,
    /// Storing the solution would exceed the output budget or the type depth bound.
    Exhausted,
}

/// Inference variables and their solutions.
///
/// Every traversal here is iterative or bounded: `head` and `last_var` follow at most
/// one link per variable; `occurs`, `unify`, and `measure_resolved` use explicit
/// work lists; `resolve` recurses only over a type whose resolved depth
/// `measure_resolved` has bounded first. Solutions form a DAG, so a resolved type can be
/// exponentially larger than what is stored; it is never materialized before its size
/// is known and charged.
#[derive(Debug, Default)]
pub(crate) struct Unifier {
    solutions: Vec<Option<Type>>,
    /// Steps taken since the elaborator last collected them ([`Self::take_work`]):
    /// one per chain link followed, type node visited, or pair unified.
    work: std::cell::Cell<u64>,
}

impl Unifier {
    /// A fresh, unsolved variable.
    pub(crate) fn fresh(&mut self) -> Type {
        let n = u32::try_from(self.solutions.len()).unwrap_or(u32::MAX);
        self.solutions.push(None);
        Type::Var(n)
    }

    /// Count `n` steps of work.
    fn step(&self, n: u64) {
        self.work.set(self.work.get().saturating_add(n));
    }

    /// The steps counted since the last call, for the elaborator to charge as fuel.
    pub(crate) fn take_work(&self) -> u64 {
        self.work.replace(0)
    }

    /// Point every variable on the solved chain starting at `t` directly at the chain's
    /// end, so later `head` calls follow one link (path compression). Charges its steps.
    fn compress(&mut self, t: &Type) {
        let mut chain: Vec<u32> = Vec::new();
        let mut current = t.clone();
        for _ in 0..=self.solutions.len() {
            let Type::Var(n) = current else {
                break;
            };
            match self.solution(n) {
                Some(Type::Var(m)) => {
                    chain.push(n);
                    current = Type::Var(*m);
                }
                _ => break,
            }
        }
        self.step(chain.len() as u64);
        if chain.len() > 1 {
            for n in chain {
                if let Some(slot) = self.solutions.get_mut(n as usize) {
                    *slot = Some(current.clone());
                }
            }
        }
    }

    fn solution(&self, n: u32) -> Option<&Type> {
        self.solutions.get(n as usize).and_then(Option::as_ref)
    }

    /// Follow solved variables at the head of `t`, by reference.
    pub(crate) fn head<'a>(&'a self, t: &'a Type) -> &'a Type {
        let mut current = t;
        // One step per solved variable; `unify` never makes a chain cyclic.
        for _ in 0..=self.solutions.len() {
            self.step(1);
            let Type::Var(n) = current else {
                return current;
            };
            match self.solution(*n) {
                Some(next) => current = next,
                None => return current,
            }
        }
        current
    }

    /// The last variable of a solved chain starting at `t`, if `t` is a variable.
    fn last_var(&self, t: &Type) -> Option<u32> {
        let mut current = t;
        let mut last = None;
        for _ in 0..=self.solutions.len() {
            self.step(1);
            let Type::Var(n) = current else {
                return last;
            };
            match self.solution(*n) {
                Some(next) => {
                    last = Some(*n);
                    current = next;
                }
                None => return last,
            }
        }
        last
    }

    /// Whether variable `n` occurs in the resolved form of `t`. Iterative; each solved
    /// variable is expanded once. A path deeper than [`crate::budget::MAX_TYPE_DEPTH`]
    /// is [`UnifyError::Exhausted`]: no pass may hold such a type, and stopping there
    /// keeps each check within the depth bound rather than the length of a chain.
    fn occurs(&self, n: u32, t: &Type) -> Result<bool, UnifyError> {
        let mut seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut stack: Vec<(&Type, usize)> = vec![(t, 1)];
        while let Some((node, depth)) = stack.pop() {
            self.step(1);
            if depth > crate::budget::MAX_TYPE_DEPTH {
                return Err(UnifyError::Exhausted);
            }
            let below = depth.saturating_add(1);
            match node {
                Type::Var(m) => {
                    if *m == n {
                        return Ok(true);
                    }
                    if seen.insert(*m)
                        && let Some(s) = self.solution(*m)
                    {
                        stack.push((s, depth));
                    }
                }
                Type::Option(a) | Type::Set(a) | Type::Seq(a) => stack.push((a, below)),
                Type::Map(a, b) | Type::Function(a, b) => {
                    stack.push((a, below));
                    stack.push((b, below));
                }
                Type::Tuple(items) => stack.extend(items.iter().map(|i| (i, below))),
                Type::Record(fields) => stack.extend(fields.iter().map(|(_, t)| (t, below))),
                _ => {}
            }
        }
        Ok(false)
    }

    /// The size and depth `resolve(t)` would have, computed without building it and
    /// without recursion. `memo` caches solved variables; it is valid while no
    /// solution changes. Sizes saturate.
    pub(crate) fn measure_resolved(
        &self,
        t: &Type,
        memo: &mut std::collections::BTreeMap<u32, (usize, usize)>,
    ) -> (usize, usize) {
        enum Work<'a> {
            Enter(&'a Type),
            Combine(usize),
            Remember(u32),
        }
        let mut work: Vec<Work<'_>> = vec![Work::Enter(t)];
        let mut values: Vec<(usize, usize)> = Vec::new();
        while let Some(w) = work.pop() {
            self.step(1);
            match w {
                Work::Enter(node) => {
                    let children: Vec<&Type> = match node {
                        Type::Var(n) => {
                            if let Some(v) = memo.get(n) {
                                values.push(*v);
                            } else if let Some(s) = self.solution(*n) {
                                work.push(Work::Remember(*n));
                                work.push(Work::Enter(s));
                            } else {
                                values.push((1, 1));
                            }
                            continue;
                        }
                        Type::Option(a) | Type::Set(a) | Type::Seq(a) => vec![a],
                        Type::Map(a, b) | Type::Function(a, b) => vec![a, b],
                        Type::Tuple(items) => items.iter().collect(),
                        Type::Record(fields) => fields.iter().map(|(_, t)| t).collect(),
                        _ => Vec::new(),
                    };
                    work.push(Work::Combine(children.len()));
                    work.extend(children.into_iter().map(Work::Enter));
                }
                Work::Combine(k) => {
                    let mut size = 1_usize;
                    let mut depth = 0_usize;
                    for _ in 0..k {
                        let (s, d) = values.pop().unwrap_or((0, 0));
                        size = size.saturating_add(s);
                        depth = depth.max(d);
                    }
                    values.push((size, depth.saturating_add(1)));
                }
                Work::Remember(n) => {
                    if let Some(v) = values.last() {
                        memo.insert(n, *v);
                    }
                }
            }
        }
        values.pop().unwrap_or((1, 1))
    }

    /// Replace every solved variable in `t`. Unsolved variables stay.
    ///
    /// Recursive over the resolved type: callers bound its depth with
    /// [`Self::measure_resolved`] and charge its size before calling.
    pub(crate) fn resolve(&self, t: &Type) -> Type {
        self.step(1);
        match self.head(t) {
            Type::Option(a) => Type::Option(Box::new(self.resolve(a))),
            Type::Set(a) => Type::Set(Box::new(self.resolve(a))),
            Type::Seq(a) => Type::Seq(Box::new(self.resolve(a))),
            Type::Map(a, b) => Type::Map(Box::new(self.resolve(a)), Box::new(self.resolve(b))),
            Type::Function(a, b) => {
                Type::Function(Box::new(self.resolve(a)), Box::new(self.resolve(b)))
            }
            Type::Tuple(items) => Type::Tuple(items.iter().map(|i| self.resolve(i)).collect()),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.resolve(t)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// A short rendering of `t` for a diagnostic: at most a few levels, `…` below.
    /// Bounded in depth and in output, whatever the resolved size of `t`.
    pub(crate) fn describe(&self, t: &Type) -> String {
        fn go(u: &Unifier, t: &Type, depth: usize, out: &mut String) {
            u.step(1);
            if depth == 0 || out.len() > 200 {
                out.push('…');
                return;
            }
            let d = depth.saturating_sub(1);
            let list = |u: &Unifier, items: &[&Type], out: &mut String| {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    go(u, item, d, out);
                }
            };
            match u.head(t) {
                Type::Option(a) => {
                    out.push_str("Option[");
                    go(u, a, d, out);
                    out.push(']');
                }
                Type::Set(a) => {
                    out.push_str("Set[");
                    go(u, a, d, out);
                    out.push(']');
                }
                Type::Seq(a) => {
                    out.push_str("Seq[");
                    go(u, a, d, out);
                    out.push(']');
                }
                Type::Map(a, b) => {
                    out.push_str("Map[");
                    list(u, &[a, b], out);
                    out.push(']');
                }
                Type::Function(a, b) => {
                    out.push('(');
                    go(u, a, d, out);
                    out.push_str(" -> ");
                    go(u, b, d, out);
                    out.push(')');
                }
                Type::Tuple(items) => {
                    out.push('(');
                    list(u, &items.iter().collect::<Vec<_>>(), out);
                    out.push(')');
                }
                Type::Record(fields) => {
                    out.push('{');
                    for (i, (n, t)) in fields.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(n);
                        out.push_str(": ");
                        go(u, t, d, out);
                    }
                    out.push('}');
                }
                other => out.push_str(&other.to_string()),
            }
        }
        let mut out = String::new();
        go(self, t, 6, &mut out);
        out
    }

    /// Store `solution` for the unsolved variable `n`, charging its structural size.
    fn bind(
        &mut self,
        n: u32,
        solution: Type,
        budget: &mut crate::budget::Budget,
    ) -> Result<(), UnifyError> {
        let (size, depth) = type_measure(&solution);
        self.step(size as u64);
        if depth > crate::budget::MAX_TYPE_DEPTH {
            return Err(UnifyError::Exhausted);
        }
        budget.charge(size).map_err(|_| UnifyError::Exhausted)?;
        if let Some(slot) = self.solutions.get_mut(n as usize) {
            *slot = Some(solution);
        }
        Ok(())
    }

    /// Make `a` and `b` equal, or report that they cannot be.
    ///
    /// Iterative over a work list of pairs. When both sides of a pair are solved
    /// variables with structures, the first is re-pointed at the second as soon as
    /// their components are queued (union-find style), so a shared subterm is
    /// unified once, not once per path to it.
    ///
    /// `Int` and `Nat` unify. A variable solved to one of them and unified with the
    /// other becomes `Nat`: the meet, so the result does not depend on the order in
    /// which the uses are met.
    pub(crate) fn unify(
        &mut self,
        a: &Type,
        b: &Type,
        budget: &mut crate::budget::Budget,
    ) -> Result<(), UnifyError> {
        let mut work: Vec<(Type, Type)> = vec![(a.clone(), b.clone())];
        while let Some((a0, b0)) = work.pop() {
            self.compress(&a0);
            self.compress(&b0);
            let ha = self.head(&a0).clone();
            let hb_size = type_measure(self.head(&b0)).0;
            self.step(type_measure(&ha).0.saturating_add(hb_size) as u64);
            let hb = self.head(&b0).clone();
            if ha.is_integer() && hb.is_integer() {
                if ha != hb {
                    for t in [&a0, &b0] {
                        if let Some(n) = self.last_var(t)
                            && let Some(slot) = self.solutions.get_mut(n as usize)
                        {
                            *slot = Some(Type::Nat);
                        }
                    }
                }
                continue;
            }
            match (&ha, &hb) {
                (Type::Var(n), Type::Var(m)) if n == m => {}
                (Type::Var(n), _) | (_, Type::Var(n)) => {
                    let n = *n;
                    let (other_original, other_head) = if matches!(ha, Type::Var(x) if x == n) {
                        (&b0, &hb)
                    } else {
                        (&a0, &ha)
                    };
                    if self.occurs(n, other_head)? {
                        return Err(UnifyError::Mismatch);
                    }
                    // Solve to the other side's own variable when it has one, so a later
                    // refinement of that variable (`Int` to `Nat`) reaches this one too.
                    let solution = match self.last_var(other_original) {
                        Some(m) if m != n => Type::Var(m),
                        _ => other_head.clone(),
                    };
                    self.bind(n, solution, budget)?;
                }
                (Type::Bool, Type::Bool) | (Type::Str, Type::Str) => {}
                (Type::Sort(x), Type::Sort(y)) | (Type::Enum(x), Type::Enum(y)) if x == y => {}
                (Type::Option(x), Type::Option(y))
                | (Type::Set(x), Type::Set(y))
                | (Type::Seq(x), Type::Seq(y)) => work.push(((**x).clone(), (**y).clone())),
                (Type::Map(k1, v1), Type::Map(k2, v2))
                | (Type::Function(k1, v1), Type::Function(k2, v2)) => {
                    work.push(((**v1).clone(), (**v2).clone()));
                    work.push(((**k1).clone(), (**k2).clone()));
                }
                (Type::Tuple(xs), Type::Tuple(ys)) if xs.len() == ys.len() => {
                    for (x, y) in xs.iter().zip(ys.iter()).rev() {
                        work.push((x.clone(), y.clone()));
                    }
                }
                (Type::Record(xs), Type::Record(ys)) if xs.len() == ys.len() => {
                    for ((nx, x), (ny, y)) in xs.iter().zip(ys.iter()).rev() {
                        if nx != ny {
                            return Err(UnifyError::Mismatch);
                        }
                        work.push((x.clone(), y.clone()));
                    }
                }
                _ => return Err(UnifyError::Mismatch),
            }
            // Union-find merge of two solved variables whose structures were just
            // matched: later pairs over the same variables are settled in one step.
            if let (Some(la), Some(lb)) = (self.last_var(&a0), self.last_var(&b0))
                && la != lb
                && !matches!(ha, Type::Var(_))
                && !matches!(hb, Type::Var(_))
                && !self.occurs(la, &hb)?
                && let Some(slot) = self.solutions.get_mut(la as usize)
            {
                *slot = Some(Type::Var(lb));
            }
        }
        Ok(())
    }
}

/// Whether a resolved type still contains an inference variable. Iterative.
pub(crate) fn has_var(t: &Type) -> bool {
    let mut stack: Vec<&Type> = vec![t];
    while let Some(node) = stack.pop() {
        match node {
            Type::Var(_) => return true,
            Type::Option(a) | Type::Set(a) | Type::Seq(a) => stack.push(a),
            Type::Map(a, b) | Type::Function(a, b) => {
                stack.push(a);
                stack.push(b);
            }
            Type::Tuple(items) => stack.extend(items.iter()),
            Type::Record(fields) => stack.extend(fields.iter().map(|(_, t)| t)),
            _ => {}
        }
    }
    false
}
