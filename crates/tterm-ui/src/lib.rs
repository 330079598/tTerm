//! The interface toolkit of the GPUI client: design tokens now, and the
//! components that replace `src/components/ui` from P0-24 on.
//!
//! `src/styles/tokens.json` is the single source of the design tokens. The
//! stylesheet the React frontend loads and the constants under [`tokens`] are
//! both generated from it, so a colour or a font size cannot drift between the
//! two clients. GPUI conversions live under [`tokens::gpui`] and receive
//! interface scale and alpha at each call site.

pub mod components;
pub mod tokens;
