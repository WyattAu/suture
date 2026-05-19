{
  description = "Suture VCS Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            sqlite
            protobuf    # For gRPC/Tonic
            cargo-audit # For security scanning in CI
          ] ++ lib.optional stdenv.isLinux [
            fuse3       # For Linux VFS
          ];

          shellHook = ''
            echo "λ Suture Development Environment Loaded"
            export RUST_BACKTRACE=1
          '';
        };

        packages = {
          default = pkgs.rustPlatform.buildRustPackage rec {
            pname = "suture";
            version = "5.1.1";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ sqlite ] ++ lib.optional stdenv.isLinux fuse3;
            cargoBuildFlags = [ "-p suture-cli" ];
            checkFlags = [ "--skip e2e" ];
          };

          suture-hub = pkgs.rustPlatform.buildRustPackage rec {
            pname = "suture-hub";
            version = "5.1.1";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };
            nativeBuildInputs = with pkgs; [ pkg-config protobuf ];
            buildInputs = with pkgs; [ sqlite openssl ];
            cargoBuildFlags = [ "-p suture-hub" ];
            checkFlags = [ "--skip e2e" ];
          };

          suture-daemon = pkgs.rustPlatform.buildRustPackage rec {
            pname = "suture-daemon";
            version = "5.1.1";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ sqlite ] ++ lib.optional stdenv.isLinux fuse3;
            cargoBuildFlags = [ "-p suture-daemon" ];
            checkFlags = [ "--skip e2e" ];
          };

          suture-lsp = pkgs.rustPlatform.buildRustPackage rec {
            pname = "suture-lsp";
            version = "5.1.0";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };
            cargoBuildFlags = [ "-p suture-lsp" ];
            checkFlags = [ "--skip e2e" ];
          };

          suture-tui = pkgs.rustPlatform.buildRustPackage rec {
            pname = "suture-tui";
            version = "5.1.1";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };
            cargoBuildFlags = [ "-p suture-tui" ];
            checkFlags = [ "--skip e2e" ];
          };
        };
      });
}
