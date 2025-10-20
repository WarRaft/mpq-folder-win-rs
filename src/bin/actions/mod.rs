// Action modules
pub mod dialog;
pub mod install_winfsp;
pub mod restart_explorer;
pub mod uninstall;

// Use WinFsp-based installer
pub use install_winfsp as install;
