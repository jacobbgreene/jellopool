{
  description = "bevy flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs =
              [
                (rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; })
                pkg-config
                gcc
                claude-code
                gemini-cli
                shell-gpt
                xset
              ]
              ++ lib.optionals (lib.strings.hasInfix "linux" system) [
                alsa-lib
                vulkan-loader
                vulkan-tools
                libudev-zero
                libx11
                libxcursor
                libxi
                libxkbcommon
                libxrandr
                wayland
              ];
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            LD_LIBRARY_PATH = lib.makeLibraryPath [
              vulkan-loader
              libx11
              libxi
              libxcursor
              libxkbcommon
              wayland
            ];
          };
      }
    );
}
