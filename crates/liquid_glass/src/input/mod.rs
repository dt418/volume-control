//! User input, hit testing, focus traversal, and IME tracking.

pub mod focus;
pub mod hit_test;
pub mod ime;

#[cfg(test)]
pub mod tests;

pub use focus::FocusManager;
pub use hit_test::hit_test;
pub use ime::{ImeAction, ImeState};
