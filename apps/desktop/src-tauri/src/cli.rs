pub use scoopy_cli_install::CliInstallStatus;

#[tauri::command]
#[specta::specta]
pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    scoopy_cli_install::status()
}

#[tauri::command]
#[specta::specta]
pub fn install_cli() -> Result<CliInstallStatus, String> {
    scoopy_cli_install::install()
}

#[tauri::command]
#[specta::specta]
pub fn uninstall_cli() -> Result<CliInstallStatus, String> {
    scoopy_cli_install::uninstall()
}
