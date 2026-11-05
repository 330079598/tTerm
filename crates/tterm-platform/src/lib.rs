//! The native platform capabilities the GPUI client injects at startup:
//! application paths, credentials, the clipboard, notifications, the tray,
//! single-instance forwarding and window operations.
//!
//! Each trait gets a Tauri implementation and a native one, so the code that
//! uses them stays free of both. The traits arrive with P1-08.
