//! A tray's lifecycle is separate from the application's close policy.
/// A client-rendered menu without native toolkit types or business state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    /// An actionable or disabled menu row, optionally with a check mark.
    Item {
        /// The stable client action identifier.
        id: String,
        /// The already-localized display text.
        label: String,
        /// Whether the user can activate the row.
        enabled: bool,
        /// None is a plain row; Some selects its check-mark state.
        checked: Option<bool>,
    },
    /// A visual separator between menu groups.
    Separator,
    /// A nested menu group.
    Submenu {
        /// The already-localized display text.
        label: String,
        /// Rows displayed when the submenu opens.
        entries: Vec<MenuEntry>,
    },
}
/// A menu presentation prepared by the application using its existing labels.
pub struct TrayModel {
    /// The ordered menu rows prepared by application policy.
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
