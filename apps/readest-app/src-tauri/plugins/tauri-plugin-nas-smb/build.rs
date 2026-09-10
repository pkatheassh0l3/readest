// Comandos que el plugin expone al frontend. Tienen que coincidir con los
// `#[command]` de src/commands.rs y con las @Command del plugin de Kotlin.
const COMMANDS: &[&str] = &[
    "connect",
    "list",
    "download",
    "upload",
    "mkdir",
    "remove",
    "disconnect",
    "tailscale_status",
    "open_tailscale",
    "uri_display_name",
    "sync_read",
    "sync_write",
    "sync_stat",
    "sync_list",
    "sync_mkdir",
    "sync_remove_dir",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
