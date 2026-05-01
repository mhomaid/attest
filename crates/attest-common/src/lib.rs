//! Shared OCSF types for the Attest platform.
//!
//! Hand-authored structs aligned to OCSF 1.3. The canonical source is
//! <https://schema.ocsf.io/>. These cover the source classes needed for Phase 1
//! (CloudTrail authentication + cloud-activity events); additional classes are
//! added per phase.

pub mod error;
pub mod flat;
pub mod ocsf;

pub use error::AttestError;
pub use flat::FlatEvent;
pub use ocsf::*;
