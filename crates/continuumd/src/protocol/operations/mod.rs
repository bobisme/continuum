//! Request and response structs, one module per IDL namespace.
//!
//! The IDL declares 74 operations in 19 namespaces (IDL §10); this module has one
//! child per namespace, in the IDL's declaration order.

pub mod benchmark;
pub mod context;
pub mod correspondence;
pub mod debug;
pub mod evidence;
pub mod failure;
pub mod forge;
pub mod intent;
pub mod model;
pub mod observe;
pub mod program;
pub mod proof;
pub mod query;
pub mod refinement;
pub mod repair;
pub mod task;
pub mod verification;
pub mod whiteboard;
pub mod workspace;
