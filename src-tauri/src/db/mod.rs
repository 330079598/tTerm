//! Database access that stays with the Tauri package until the one-time JSON
//! import moves; everything else lives in [`tterm_core::db`].

pub(crate) mod legacy_import;
