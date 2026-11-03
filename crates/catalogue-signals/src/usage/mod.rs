//! Adoption signals from reverse dependencies: active external users, their
//! monthly history, former glory, growth and new adopters.
//!
//! The measure follows the reverse-dependency traction idea used by lib.rs.

mod compute;
mod date;
mod lifetime;
mod runs;
mod series;
mod trend;
mod uses;
mod weight;

pub use compute::compute;
pub use series::Usage;
