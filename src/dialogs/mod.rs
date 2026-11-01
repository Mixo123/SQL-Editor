//! Modal dialogs for connections, settings, about, and updates.

pub mod about;
pub mod connection;
pub mod settings;
pub mod update;

pub use about::AboutDialog;
pub use connection::ConnectionDialog;
pub use settings::{load_settings, AppSettings, SettingsDialog};
pub use update::UpdateDialog;
