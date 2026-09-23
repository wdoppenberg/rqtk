//! **rqtk**: requirements toolkit.
//!
//! This crate is mainly the `rqtk` command-line tool; install it with
//! `cargo install rqtk --locked` or see <https://rqtk.dev>. Its library side is a thin facade
//! over the crates the tool is built from.
//!
//! # Linking tests to requirements
//!
//! Enable the `macros` feature to annotate Rust tests with the verification activity they
//! prove. An unknown activity ID is a compile error, and the annotated test gets the
//! requirement's text as its documentation.
//!
//! ```toml
//! [dev-dependencies]
//! rqtk = { version = "1", default-features = false, features = ["macros"] }
//! ```
//!
//! ```rust,ignore
//! #[rqtk::verifies("VA-SYS-001-01")]
//! #[test]
//! fn boots_in_under_five_seconds() {
//!     // …
//! }
//! ```
//!
//! # Stability
//!
//! The modules below re-export crates that are still 0.x, so they are not covered by rqtk's
//! 1.x stability promise; the `#[verifies("…")]` attribute itself is.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use rqtk_core as core;
pub use rqtk_export as export;

/// Attribute macros that link Rust code to requirements (`rqtk-macros`).
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use rqtk_macros as macros;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use rqtk_macros::{requirements_docs, verifies};
