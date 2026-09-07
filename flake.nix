{
  description = "bevy flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            # Rust toolchain (incl. rust-analyzer + rust-src) comes from the
            # system config (rust-overlay), not pinned here, so rust-analyzer
            # always matches rustc. This shell only supplies the Bevy system
            # libraries + LD_LIBRARY_PATH the build/run needs.
            buildInputs =
              [
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
