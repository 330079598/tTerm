//! The compose bar and the line-by-line runs it sends, from
//! `src/lib/commandComposer.ts` and `src/lib/composerRun.ts`.
//!
//! The bar's React store, its per-tab terminal registry and the prompt subscriptions stay
//! in the WebView; the pure text and run-state conversions live here.

pub mod input;
pub mod run;
#[cfg(test)]
mod tests;
