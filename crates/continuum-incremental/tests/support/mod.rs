//! Shared fixtures: the Die Hard corpus model and its CML mutation corpus.

#![allow(dead_code)]

/// The TV-009 Die Hard corpus model.
pub fn diehard() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm"
    );
    std::fs::read_to_string(path).expect("the corpus file is present")
}

/// One CML edit.
pub struct Mutation {
    /// Stable id.
    pub id: &'static str,
    /// The corpus class.
    pub kind: &'static str,
    /// What it does.
    pub description: &'static str,
    /// The edits: `(pattern, replacement)`, every occurrence. Each pattern must occur.
    pub edits: &'static [(&'static str, &'static str)],
}

impl Mutation {
    /// Apply to `base`. Panics when a pattern does not occur, so an edit that
    /// silently stopped applying cannot pass as a no-op.
    pub fn apply(&self, base: &str) -> String {
        let mut out = base.to_owned();
        for (pattern, replacement) in self.edits {
            assert!(
                out.contains(pattern),
                "{}: pattern {pattern:?} does not occur",
                self.id
            );
            out = out.replace(pattern, replacement);
        }
        assert_ne!(out, base, "{}: the edit changed nothing", self.id);
        out
    }
}

/// The mutation corpus: whitespace-only, comment, rename, semantic change, and
/// property change edits, plus two edits aimed at the `Experimental` heuristic and
/// one parse failure.
pub const CORPUS: &[Mutation] = &[
    Mutation {
        id: "ws-reindent",
        kind: "whitespace",
        description: "re-indent every action and add a blank line before each",
        edits: &[("\n  action", "\n\n      action")],
    },
    Mutation {
        id: "ws-trailing",
        kind: "whitespace",
        description: "trailing spaces after every closing brace",
        edits: &[("}\n", "}   \n")],
    },
    Mutation {
        id: "comment-add",
        kind: "comment",
        description: "a new line comment before the init block",
        edits: &[("  init Init", "  // the jugs start empty\n  init Init")],
    },
    Mutation {
        id: "comment-edit",
        kind: "comment",
        description: "shorten the trailing comment on NotSolved",
        edits: &[(
            "// intentionally false; shortest witness is the solution",
            "// intentionally false",
        )],
    },
    Mutation {
        id: "rename-let",
        kind: "rename",
        description: "rename the let binding next_big to nb",
        edits: &[("next_big", "nb")],
    },
    Mutation {
        id: "rename-invariant",
        kind: "rename",
        description: "rename the invariant NotSolved to BigNotFour",
        edits: &[("NotSolved", "BigNotFour")],
    },
    Mutation {
        id: "rename-action",
        kind: "rename",
        description: "rename the action FillSmall to FillSmallJug everywhere",
        edits: &[("FillSmall", "FillSmallJug")],
    },
    Mutation {
        id: "prop-edit",
        kind: "property",
        description: "NotSolved checks big != 3 instead of big != 4",
        edits: &[("{ big != 4 }", "{ big != 3 }")],
    },
    Mutation {
        id: "prop-add",
        kind: "property",
        description: "add the invariant SmallCapped",
        edits: &[(
            "  invariant TypeOK",
            "  invariant SmallCapped { small <= 3 }\n  invariant TypeOK",
        )],
    },
    Mutation {
        id: "prop-remove",
        kind: "property",
        description: "remove the invariant TypeOK",
        edits: &[("  invariant TypeOK { big in 0..5 && small in 0..3 }\n", "")],
    },
    Mutation {
        id: "sem-domain-widen",
        kind: "semantic",
        description: "widen big's domain to 0..6 (the reachable states do not change)",
        edits: &[("big: Nat where big <= 5", "big: Nat where big <= 6")],
    },
    Mutation {
        id: "sem-fillbig-4",
        kind: "semantic",
        description: "FillBig fills big to 4 instead of 5",
        edits: &[("action FillBig { big' == 5", "action FillBig { big' == 4")],
    },
    Mutation {
        id: "sem-emptybig-noop",
        kind: "semantic",
        description: "EmptyBig no longer empties big (fewer reachable states)",
        edits: &[(
            "action EmptyBig { big' == 0 && small' == small }",
            "action EmptyBig { big' == big && small' == small }",
        )],
    },
    Mutation {
        id: "sem-domain-narrow",
        kind: "semantic",
        description: "narrow small's domain to 0..2, below what FillSmall writes",
        edits: &[("small: Nat where small <= 3", "small: Nat where small <= 2")],
    },
    Mutation {
        id: "trap-join-lines",
        kind: "heuristic-trap",
        description: "join two statements of SmallToBig onto one line (line breaks separate statements)",
        edits: &[(
            "let next_big = min(big + small, 5)\n    big' == next_big",
            "let next_big = min(big + small, 5) big' == next_big",
        )],
    },
    Mutation {
        id: "trap-split-expression",
        kind: "heuristic-trap",
        description: "break the expression big' == next_big across two lines",
        edits: &[("big' == next_big", "big' ==\n      next_big")],
    },
    Mutation {
        id: "parse-error",
        kind: "syntax",
        description: "drop the model's closing brace",
        edits: &[(
            "shortest witness is the solution\n}",
            "shortest witness is the solution\n",
        )],
    },
];
