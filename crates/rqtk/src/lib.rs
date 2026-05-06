pub use rqtk_core as core;

#[cfg(feature = "macros")]
pub use rqtk_macros as macros;

#[cfg(feature = "macros")]
pub use rqtk_macros::{requirements_docs, verifies};

#[cfg(feature = "macros")]
#[requirements_docs]
pub mod requirements {}
