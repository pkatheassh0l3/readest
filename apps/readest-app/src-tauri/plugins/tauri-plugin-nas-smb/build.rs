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
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
