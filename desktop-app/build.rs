// Suture Desktop Application
//
// This is a Tauri v2 scaffold. To build:
// 1. Install system dependencies:
//    - Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
//    - Fedora: sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel
//    - Nix: add pkgs.webkitgtk_4_1 pkgs.gtk3 pkgs.libappindicator-gtk3 pkgs.librsvg to environment
// 2. cargo build --features tauri
// 3. cargo tauri dev

fn main() {
    #[cfg(feature = "tauri")]
    {
        tauri_build::build()
    }
    #[cfg(not(feature = "tauri"))]
    {
        println!("cargo:warning=suture-desktop requires the 'tauri' feature to build.");
        println!("cargo:warning=Install system deps and rebuild with --features tauri");
    }
}
