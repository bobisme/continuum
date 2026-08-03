//! Routing evidence for the composed certificate surface (plan §20; INV-004; INV-008).
//!
//! > proof/certificate checker does not depend on search engines; … a certificate is
//! > checked from its wire form, never from shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! An integration test sees exactly what a consumer sees: the public surface, with no
//! access to any crate's `cfg(test)` fixtures. Every certificate below is therefore
//! built as *bytes*, by a hand-written encoder in this file, from the grammars the four
//! kernels document in their own `wire` modules. The four green fixtures are the ones
//! each kernel's own `tests/wire_form_boundary.rs` already carries, byte for byte, and
//! for the same reason those files give: a checker that is handed bytes produced by its
//! own encoder has been tested against itself (docs/03 §8, "diversity against
//! common-mode bugs").
//!
//! What this file establishes, in the order the claims are made:
//!
//! 1. **The routing table is the kernels' own.** `Family::magic` returns each kernel's
//!    `wire::MAGIC`, the four are pairwise distinct, and `from_magic` inverts `magic`
//!    exactly.
//! 2. **Routing is correct per format.** Each family's green certificate reaches that
//!    family's checker and verifies from bytes alone.
//! 3. **The verdict vocabulary is preserved, not restated.** For green, rejected and
//!    unsupported artifacts of all four families, the composed answer is *equal* to what
//!    the routed kernel's own `check_certificate` returns on the same bytes. Nothing is
//!    re-mapped, so nothing can drift.
//! 4. **Routing does not decide.** A one-byte mutation anywhere past the magic leaves
//!    the route alone and changes the verdict — the anti-vacuity half: green verdicts
//!    are not a property of having been routed.
//! 5. **A routing failure is not a verdict.** Short input and an unclaimed magic are
//!    `Unroutable`, never `Rejected` and never `Unsupported`.
//! 6. **There is no fallback.** A certificate stamped with another family's magic is
//!    handed to the kernel it names, which refuses it; no second checker is tried.
//! 7. **Garbage is data.** Every prefix of every fixture, and 256 filler patterns, and
//!    the empty string: no panic, no `Verified`.

use continuum_certificate::{
    Family, KernelVerdict, MAGIC_BYTES, Outcome, RoutingFault, check_certificate,
};

// --- encoders, written against the kernels' documented grammars ------------------------

/// A minimal encoder. Shares no code with any decoder under test.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u64(&mut self, value: u64) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn i32(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn i64(&mut self, value: i64) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn token(&mut self, value: &str) -> &mut Self {
        self.u16(u16::try_from(value.len()).expect("fixture tokens are short"));
        self.0.extend_from_slice(value.as_bytes());
        self
    }

    fn clause(&mut self, literals: &[i32]) -> &mut Self {
        self.u32(u32::try_from(literals.len()).expect("fixture clauses are short"));
        for literal in literals {
            self.i32(*literal);
        }
        self
    }

    /// Header plus the eight-field RFC 0005 claim envelope, which every family shares.
    fn header(&mut self, family: Family, kind: u16, model: &str, property: &str) -> &mut Self {
        self.0.extend_from_slice(&family.magic());
        self.u16(1); // wire epoch; all four kernels are at epoch 1
        self.u16(kind);
        self.token(model);
        self.token("continuum-semantics-1");
        self.token(property);
        self.token("blake3:fixture-scope");
        self.token("blake3:empty-assumptions");
        self.token("continuum-producer/0.0.0");
        self.u16(1); // schema epoch, which must agree with the header
        self.u16(0) // no domain packs
    }
}

/// `continuum-kernel-core`, `CONTCERT`: the one-bit toggle of that crate's own
/// `tests/wire_form_boundary.rs` — states `[0]` and `[1]`, one action `flip`, closed.
fn core_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(
        Family::Core,
        1, // finite-closure
        "blake3:toggle-model",
        "blake3:toggle-property",
    );

    out.u16(1); // one variable
    out.token("bit");
    out.i64(0);
    out.i64(1);

    out.u32(2); // two states
    out.i64(0);
    out.i64(1);

    out.u16(1); // property class: state-domain
    out.u32(1); // one initial state
    out.i64(0);
    out.u16(1); // one action
    out.token("flip");

    out.u32(1); // row for state [0]
    out.u16(0);
    out.i64(1);
    out.u32(1); // row for state [1]
    out.u16(0);
    out.i64(0);

    out.0
}

/// `continuum-kernel-sat`, `CONTSATC`: `(x) ∧ (¬x)`, refuted by one two-antecedent
/// chain.
fn sat_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(
        Family::Sat,
        1, // lrat
        "blake3:contradiction-model",
        "blake3:contradiction-property",
    );

    out.u32(1); // one variable
    out.u32(2); // two input clauses
    out.clause(&[1]); // id 1: (x)
    out.clause(&[-1]); // id 2: (¬x)

    out.u32(1); // one proof step
    out.u16(1); // add
    out.u32(3); // id 3
    out.clause(&[]); // the empty clause
    out.u32(2); // two antecedents
    out.u32(1);
    out.u32(2);

    out.0
}

/// `continuum-kernel-smt`, `CONTSMTC`: `a < b` and `b < a`, refuted by one `LIA`
/// antisymmetry lemma.
fn smt_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(
        Family::Smt,
        1, // smt-proof
        "blake3:boundary-model",
        "blake3:boundary-property",
    );

    out.u32(2); // two atoms
    out.token("a-lt-b");
    out.token("b-lt-a");

    out.u32(2); // assertions: ids 1, 2
    out.clause(&[1]);
    out.clause(&[2]);
    out.u32(1); // lemmas: id 3
    out.token("LIA");
    out.clause(&[-1, -2]);

    out.u32(1); // one proof step
    out.u16(1); // resolve
    out.u32(4); // id 4
    out.clause(&[]); // the empty clause
    out.u32(3);
    out.u32(1);
    out.u32(2);
    out.u32(3);

    out.0
}

/// `continuum-kernel-temporal`, `CONTTMPC`: the countdown `n ∈ 0..2` with a ranking
/// witness for `eventually n = 0`.
fn temporal_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(
        Family::Temporal,
        1, // ranking
        "blake3:countdown-model",
        "blake3:eventually-zero",
    );

    out.u16(1); // one variable
    out.token("n");
    out.i64(0);
    out.i64(2);

    out.u32(3); // three states
    out.i64(0);
    out.i64(1);
    out.i64(2);

    out.u16(1); // property class: eventually-state-set

    out.u32(1); // one goal state
    out.i64(0);

    out.u32(1); // one initial state
    out.i64(2);

    out.u16(1); // one action
    out.token("tick");

    out.u16(1); // weak fairness
    out.u16(0); // no fair actions

    out.u32(0); // row for n = 0: the goal, no transitions
    out.u32(1); // row for n = 1
    out.u16(0);
    out.i64(0);
    out.u32(1); // row for n = 2
    out.u16(0);
    out.i64(1);

    out.u64(0); // rank(0)
    out.u64(1); // rank(1)
    out.u64(2); // rank(2)

    out.0
}

/// Every family with its green fixture, so each claim below sweeps all four.
fn green_fixtures() -> [(Family, Vec<u8>); 4] {
    [
        (Family::Core, core_certificate()),
        (Family::Sat, sat_certificate()),
        (Family::Smt, smt_certificate()),
        (Family::Temporal, temporal_certificate()),
    ]
}

/// Whether a relayed verdict is its kernel's `Verified` arm.
///
/// Written *here*, in a test, on purpose: the library exposes no such collapse, and
/// this file is where the loss is allowed to be, visible in six lines.
fn is_verified(verdict: &KernelVerdict) -> bool {
    match verdict {
        KernelVerdict::Core(inner) => inner.is_verified(),
        KernelVerdict::Sat(inner) => inner.is_verified(),
        KernelVerdict::Smt(inner) => inner.is_verified(),
        KernelVerdict::Temporal(inner) => inner.is_verified(),
    }
}

/// What the routed kernel itself answers on the same bytes.
fn kernel_answer(family: Family, bytes: &[u8]) -> KernelVerdict {
    match family {
        Family::Core => KernelVerdict::Core(
            continuum_certificate::continuum_kernel_core::check_certificate(bytes),
        ),
        Family::Sat => KernelVerdict::Sat(
            continuum_certificate::continuum_kernel_sat::check_certificate(bytes),
        ),
        Family::Smt => KernelVerdict::Smt(
            continuum_certificate::continuum_kernel_smt::check_certificate(bytes),
        ),
        Family::Temporal => KernelVerdict::Temporal(
            continuum_certificate::continuum_kernel_temporal::check_certificate(bytes),
        ),
    }
}

// --- claim 1: the routing table is the kernels' own -----------------------------------

#[test]
fn the_routing_table_is_the_kernels_own_magics_and_they_partition_the_input_space() {
    assert_eq!(Family::ALL.len(), 4, "one family per kernel crate, no more");

    // The magic each family routes by is the constant the owning kernel exports; this
    // crate declares none of its own, so the table cannot drift from the format.
    assert_eq!(
        Family::Core.magic(),
        continuum_certificate::continuum_kernel_core::wire::MAGIC
    );
    assert_eq!(
        Family::Sat.magic(),
        continuum_certificate::continuum_kernel_sat::wire::MAGIC
    );
    assert_eq!(
        Family::Smt.magic(),
        continuum_certificate::continuum_kernel_smt::wire::MAGIC
    );
    assert_eq!(
        Family::Temporal.magic(),
        continuum_certificate::continuum_kernel_temporal::wire::MAGIC
    );

    // Pairwise distinct: two families sharing a magic would make routing a coin flip,
    // and the first one listed would silently own the other's artifacts.
    for (index, left) in Family::ALL.iter().enumerate() {
        for right in Family::ALL.iter().skip(index + 1) {
            assert_ne!(
                left.magic(),
                right.magic(),
                "{left:?} and {right:?} claim the same magic"
            );
        }
        assert_eq!(left.magic().len(), MAGIC_BYTES);
        // `from_magic` inverts `magic` exactly, for every family.
        assert_eq!(Family::from_magic(&left.magic()), Some(*left));
    }

    // And claims nothing else.
    assert_eq!(Family::from_magic(b"CONTXXXX"), None);
    assert_eq!(Family::from_magic(&[0_u8; MAGIC_BYTES]), None);
    assert_eq!(Family::from_magic(b"contcert"), None, "magic is case-exact");

    assert_eq!(Family::Core.checker_crate(), "continuum-kernel-core");
    assert_eq!(Family::Sat.checker_crate(), "continuum-kernel-sat");
    assert_eq!(Family::Smt.checker_crate(), "continuum-kernel-smt");
    assert_eq!(
        Family::Temporal.checker_crate(),
        "continuum-kernel-temporal"
    );
}

// --- claim 2: routing is correct per format -------------------------------------------

#[test]
fn each_family_reaches_its_own_checker_and_verifies_from_wire_bytes_alone() {
    for (family, bytes) in green_fixtures() {
        let outcome = check_certificate(&bytes);
        let Outcome::Checked(verdict) = &outcome else {
            panic!("{family:?}'s green certificate did not reach a checker: {outcome:?}");
        };
        assert_eq!(
            verdict.family(),
            family,
            "{family:?}'s certificate was routed to {:?}",
            verdict.family()
        );
        assert!(
            is_verified(verdict),
            "{family:?}'s green certificate did not verify: {verdict:?}"
        );
        // The accessors agree with the match, and `route` agrees with the dispatcher.
        assert_eq!(outcome.family(), Some(family));
        assert_eq!(outcome.routing_fault(), None);
        assert_eq!(Family::route(&bytes), Ok(family));
    }
}

// --- claim 3: the verdict vocabulary is preserved, not restated -----------------------

#[test]
fn the_relayed_verdict_is_the_kernels_own_value_for_every_arm_of_the_vocabulary() {
    for (family, green) in green_fixtures() {
        // Verified.
        assert_eq!(
            check_certificate(&green),
            Outcome::Checked(kernel_answer(family, &green)),
            "{family:?}: a green verdict was not relayed verbatim"
        );

        // Rejected: a trailing byte is a second encoding of the same claim, which every
        // kernel refuses (RFC 0005, "Resource bounds").
        let mut trailing = green.clone();
        trailing.push(0x00);
        let relayed = check_certificate(&trailing);
        assert_eq!(
            relayed,
            Outcome::Checked(kernel_answer(family, &trailing)),
            "{family:?}: a rejection was not relayed verbatim"
        );
        let Outcome::Checked(verdict) = &relayed else {
            panic!("{family:?}: a trailing byte must still reach the checker");
        };
        assert!(!is_verified(verdict), "{family:?}: {verdict:?}");

        // Unsupported: the wire epoch is a `u16` immediately after the magic in all four
        // grammars, and every kernel answers a foreign epoch with `Unsupported`, never
        // `Rejected` — the artifact may be valid under a contract this build has never
        // seen (INV-008). That distinction is what a composition is most likely to lose,
        // so it is asserted by *identity* with the kernel's own answer and then named.
        let mut foreign_epoch = green.clone();
        foreign_epoch
            .get_mut(MAGIC_BYTES..MAGIC_BYTES + 2)
            .expect("a green certificate carries a wire epoch")
            .copy_from_slice(&u16::MAX.to_be_bytes());
        let relayed = check_certificate(&foreign_epoch);
        assert_eq!(
            relayed,
            Outcome::Checked(kernel_answer(family, &foreign_epoch)),
            "{family:?}: an unsupported feature was not relayed verbatim"
        );
        let unsupported = match relayed {
            Outcome::Checked(KernelVerdict::Core(inner)) => matches!(
                inner,
                continuum_certificate::continuum_kernel_core::Verdict::Unsupported(
                    continuum_certificate::continuum_kernel_core::Feature::WireEpoch {
                        found: u16::MAX
                    }
                )
            ),
            Outcome::Checked(KernelVerdict::Sat(inner)) => matches!(
                inner,
                continuum_certificate::continuum_kernel_sat::Verdict::Unsupported(
                    continuum_certificate::continuum_kernel_sat::Feature::WireEpoch {
                        found: u16::MAX
                    }
                )
            ),
            Outcome::Checked(KernelVerdict::Smt(inner)) => matches!(
                inner,
                continuum_certificate::continuum_kernel_smt::Verdict::Unsupported(
                    continuum_certificate::continuum_kernel_smt::Feature::WireEpoch {
                        found: u16::MAX
                    }
                )
            ),
            Outcome::Checked(KernelVerdict::Temporal(inner)) => matches!(
                inner,
                continuum_certificate::continuum_kernel_temporal::Verdict::Unsupported(
                    continuum_certificate::continuum_kernel_temporal::Feature::WireEpoch {
                        found: u16::MAX
                    }
                )
            ),
            Outcome::Unroutable(fault) => {
                panic!("{family:?}: epoch patch broke routing: {fault:?}")
            }
        };
        assert!(
            unsupported,
            "{family:?}: a foreign wire epoch must surface as Unsupported through the \
             composition, never weakened into a rejection"
        );
    }
}

// --- claim 4: routing does not decide (anti-vacuity) ----------------------------------

/// The byte positions of each green fixture whose inversion the routed kernel still
/// verifies — enumerated, so that "the sweep has survivors" is a stated fact with a
/// reason rather than a tolerance.
///
/// None of these is a hole in a checker. Each is a **bound**, a **capacity**, or an
/// **identifier**: a declaration that inverting *loosens*, and a certificate whose
/// declaration is looser makes a weaker claim about the same content, which is still
/// true. Every other byte of every fixture carries content, and inverting any one of
/// them loses the verdict.
///
/// - `Core` — `165` is the most significant byte of the declared lower bound of `bit`,
///   sending it hugely negative; `174..=180` are the seven low bytes of its declared
///   upper bound, sending that hugely positive. Both widen the declared state domain
///   `P`, and `S ⊆ P` survives a wider `P`. The most significant byte of the *upper*
///   bound is not here: inverting it makes `hi` negative, `lo > hi`, and the range
///   inverted — a rejection.
/// - `Sat` — `174..=175` are the low half of the declared variable count; declaring
///   more variables than the clauses mention is legal and changes no propagation.
///   `203..=205` are three bytes of the derived clause's identifier; an id is a name,
///   and renaming the empty clause to a larger unused id leaves the single
///   two-antecedent chain exactly as it was.
/// - `Smt` — `230..=232` are the same three bytes of the resolvent step's identifier,
///   for the same reason.
/// - `Temporal` — `166` and `175..=181` are the declared range of `n`, widened, as in
///   `Core`. `296..=303` are all eight bytes of `rank(2)`: inverting any byte of `2`
///   only makes the rank larger, and a ranking function has to *decrease* along the
///   transition it justifies, so a larger rank at the source is still a valid witness.
const VERDICT_INDEPENDENT_BYTES: [(Family, &[usize]); 4] = [
    (Family::Core, &[165, 174, 175, 176, 177, 178, 179, 180]),
    (Family::Sat, &[174, 175, 203, 204, 205]),
    (Family::Smt, &[230, 231, 232]),
    (
        Family::Temporal,
        &[
            166, 175, 176, 177, 178, 179, 180, 181, 296, 297, 298, 299, 300, 301, 302, 303,
        ],
    ),
];

#[test]
fn a_single_byte_mutation_past_the_magic_keeps_the_route_and_the_kernels_own_answer() {
    // The anti-vacuity half of claims 2 and 3. Every byte of every fixture after the
    // magic is inverted in turn, and three things are asserted at every position:
    //
    //   * the artifact still reaches the *same* checker — routing depends on the magic
    //     and on nothing else, so a corrupted body cannot be re-routed to a kernel that
    //     happens to like it;
    //   * the composed answer is still exactly the routed kernel's own answer — the
    //     relay is transparent under corruption too, not only on the happy path;
    //   * the verdict is lost, except at the enumerated bound/capacity/identifier bytes
    //     above.
    //
    // The last of the three is what makes the green verdicts of claim 2 mean something:
    // if a fixture verified no matter what its bytes said, "routed and verified" would
    // be a statement about the dispatcher rather than about the certificate.
    for (family, green) in green_fixtures() {
        let expected_slack = VERDICT_INDEPENDENT_BYTES
            .iter()
            .find(|(listed, _)| *listed == family)
            .map(|(_, positions)| *positions)
            .expect("every family has an entry, even if it is empty");

        let mut survivors: Vec<usize> = Vec::new();
        for position in MAGIC_BYTES..green.len() {
            let mut bytes = green.clone();
            let byte = bytes
                .get_mut(position)
                .expect("position is inside the fixture");
            *byte ^= 0xFF;

            let outcome = check_certificate(&bytes);
            assert_eq!(
                outcome,
                Outcome::Checked(kernel_answer(family, &bytes)),
                "{family:?}: at byte {position} the composition stopped relaying the \
                 kernel's own answer"
            );
            let Outcome::Checked(verdict) = &outcome else {
                panic!("{family:?}: mutating byte {position} broke routing: {outcome:?}");
            };
            assert_eq!(
                verdict.family(),
                family,
                "{family:?}: mutating byte {position} changed the route"
            );
            if is_verified(verdict) {
                survivors.push(position);
            }
        }

        assert_eq!(
            survivors, expected_slack,
            "{family:?}: the set of bytes the verdict does not depend on moved. Either a \
             kernel changed what it accepts, or a content byte became optional — read the \
             difference before widening the list"
        );
        let decisive = green.len().saturating_sub(MAGIC_BYTES) - survivors.len();
        assert!(
            decisive > 64,
            "{family:?}: only {decisive} bytes were decisive; the fixture is too small \
             for this sweep to mean anything"
        );
    }
}

// --- claim 5: a routing failure is not a verdict --------------------------------------

#[test]
fn bytes_too_short_to_carry_a_magic_reach_no_checker() {
    for length in 0..MAGIC_BYTES {
        let bytes = vec![b'C'; length];
        assert_eq!(
            check_certificate(&bytes),
            Outcome::Unroutable(RoutingFault::TooShortForMagic { available: length }),
            "a {length}-byte input cannot name a family and must not be judged as one"
        );
        assert_eq!(
            Family::route(&bytes),
            Err(RoutingFault::TooShortForMagic { available: length })
        );
    }
}

#[test]
fn an_unclaimed_magic_is_a_routing_fault_carrying_the_bytes_that_arrived() {
    // Any of the eight magic bytes, inverted, leaves an eight-byte header no kernel
    // claims. The fault reports what arrived, not what was expected: a caller debugging
    // a corrupt artifact needs the former.
    for (_, green) in green_fixtures() {
        for position in 0..MAGIC_BYTES {
            let mut bytes = green.clone();
            let byte = bytes.get_mut(position).expect("the magic is present");
            *byte ^= 0xFF;
            let expected: [u8; MAGIC_BYTES] = bytes
                .get(..MAGIC_BYTES)
                .and_then(|head| <[u8; MAGIC_BYTES]>::try_from(head).ok())
                .expect("the mutated fixture still has eight leading bytes");

            assert_eq!(
                check_certificate(&bytes),
                Outcome::Unroutable(RoutingFault::UnknownFamily { magic: expected }),
                "a magic no kernel claims must be Unroutable, not Rejected and not \
                 Unsupported: this crate has no decoder and cannot tell those apart"
            );
        }
    }
}

// --- claim 6: no fallback across checkers ---------------------------------------------

#[test]
fn a_certificate_stamped_with_another_familys_magic_is_refused_by_the_kernel_it_names() {
    // The routing decision is the artifact's own claim about which contract it is
    // written against, and it is honoured literally. A `CONTSATC` body under a
    // `CONTCERT` header goes to `continuum-kernel-core`, which refuses it; the
    // dispatcher does not retry, does not sniff the body, and does not fall back to the
    // kernel that would have understood it. Anything else would let a producer smuggle
    // an artifact past the checker its own header names.
    for (family, green) in green_fixtures() {
        for other in Family::ALL {
            if other == family {
                continue;
            }
            let mut bytes = green.clone();
            bytes
                .get_mut(..MAGIC_BYTES)
                .expect("the magic is present")
                .copy_from_slice(&other.magic());

            let outcome = check_certificate(&bytes);
            let Outcome::Checked(verdict) = &outcome else {
                panic!("{family:?} restamped as {other:?} did not reach a checker: {outcome:?}");
            };
            assert_eq!(
                verdict.family(),
                other,
                "restamping must route by the magic that is actually there"
            );
            assert!(
                !is_verified(verdict),
                "{other:?} verified a {family:?} certificate wearing its magic: {verdict:?}"
            );
        }
    }
}

// --- claim 7: garbage is data ----------------------------------------------------------

#[test]
fn garbage_never_verifies_and_never_panics_through_the_composition() {
    // docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker, and
    // a dispatcher in front of the kernel is inside that blast radius. Reaching the end
    // of this test is the no-panic half; the assertions are the no-false-green half.
    for (_, green) in green_fixtures() {
        assert!(
            green.len() < 8192,
            "the fixtures stay small enough to sweep"
        );
        for cut in 0..green.len() {
            let prefix = green.get(..cut).expect("cut is within the certificate");
            if let Outcome::Checked(verdict) = check_certificate(prefix) {
                assert!(!is_verified(&verdict), "a {cut}-byte prefix was accepted");
            }
        }
    }

    for byte in 0..=u8::MAX {
        let filler = vec![byte; 97];
        if let Outcome::Checked(verdict) = check_certificate(&filler) {
            assert!(!is_verified(&verdict), "97 bytes of {byte:#04x} verified");
        }
    }

    assert_eq!(
        check_certificate(&[]),
        Outcome::Unroutable(RoutingFault::TooShortForMagic { available: 0 })
    );
}
