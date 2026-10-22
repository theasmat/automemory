//! Continuity: what a long chat with one agent hands to the next. C1: the brief (store, delivery); readers and the
//! chat archive come in later phases (docs/specs/continuity.md).
pub mod archive;
pub mod deliver;
pub mod drafter;
pub mod history_sync;
pub mod model;
pub mod readers;
pub mod store;
pub mod import;

pub use model::{BriefItem, ItemKind, ItemState, ItemStatus, NewItem, Origin};
