#![doc = include_str!("../README.md")]

mod engine;
mod error;
mod fact;
mod macros;
mod rule;

#[cfg(feature = "visualization")]
pub mod visualization;

pub use engine::{Engine, Evaluation, RuleStatus, RuleTrace};
pub use error::Error;
pub use fact::{Fact, FactSpec, Facts};
pub use rule::{Inputs, Rule};
