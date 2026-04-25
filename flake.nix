{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };
  outputs =
    { self, nixpkgs }:
    let
      pkgs = nixpkgs.legacyPackages."x86_64-linux";
    in
    {
      devShells."x86_64-linux".default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pkg-config
          wrapGAppsHook4
          rust-analyzer
          clippy
          rustfmt
          cargo
          cargo-tauri # Optional, Only needed if Tauri doesn't work through the traditional way.
          bun # Optional, this is for if you have a js frontend
          rustc # Needed for dev server (npm tauri dev)
        ];

        buildInputs = with pkgs; [
          librsvg
          webkitgtk_4_1
        ];

        shellHook = ''
          export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH" # Needed on Wayland to report the correct display scale
        '';
      };
    };
}
