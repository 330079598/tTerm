//! A tray's lifecycle is separate from the application's close policy.
/// A client-rendered menu without native toolkit types or business state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: String,
        label: String,
        enabled: bool,
        checked: Option<bool>,
    },
    Separator,
    Submenu {
        label: String,
        entries: Vec<MenuEntry>,
    },
}
/// A menu presentation prepared by the application using its existing labels.
pub struct TrayModel {
    pub entries: Vec<MenuEntry>,
}
/// Lifecycle operations dispatch to the owning UI thread.
pub trait Tray: Send + Sync {
    /// Installs or updates the prepared menu.
    fn present(&self, model: TrayModel) -> Result<(), String>;
    /// Schedules removal on the owning UI thread; success acknowledges dispatch.
    /// Until removal completes, is_present is false and a later present must
    /// be applied after removal rather than updating the outgoing icon.
    fn remove(&self) -> Result<(), String>;
    /// Reports usable installation, excluding an icon awaiting removal.
    fn is_present(&self) -> bool;
    /// Updates the native tray tooltip.
    fn set_tooltip(&self, text: String) -> Result<(), String>;
}
