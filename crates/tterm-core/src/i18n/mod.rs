//! The shared translation catalogue and its language rules.
//!
//! The JSON files under `src/i18n/locales/` stay the only source of user-facing
//! text. `catalogue` embeds and validates them, `translate` owns the key and
//! argument types used by both clients, and `language` holds the supported
//! languages and their plural rules.

pub mod catalogue;
pub mod keys;
pub mod language;
pub mod translate;

#[cfg(test)]
mod tests;
