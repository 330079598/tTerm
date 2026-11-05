//! The interface toolkit of the GPUI client: design tokens now, and the
//! components that replace `src/components/ui` from P0-24 on.
//!
//! `src/styles/tokens.json` is the single source of the design tokens. The
//! stylesheet the React frontend loads and the constants under [`tokens`] are
//! both generated from it, so a colour or a font size cannot drift between the
//! two clients. Nothing here depends on GPUI yet, which keeps the crate
//! buildable before P0-06 fixes the GPUI revision.

pub mod tokens;
