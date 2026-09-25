//! The C005 test support, loaded once for the crate's unit tests: the generated
//! corpus, the adversarial models, and the oracle comparison, shared with the
//! integration tests under `tests/support/`.
//!
//! Evidence for RFC 0004 correction 1 and claim C005.

#[path = "../tests/support/corpus.rs"]
pub(crate) mod corpus;
#[path = "../tests/support/differential.rs"]
pub(crate) mod differential;
#[path = "../tests/support/handmade.rs"]
pub(crate) mod handmade;
