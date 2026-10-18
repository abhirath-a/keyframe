{
  description = "rust devshell";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "keyframe";
            version = "0.1.0";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = with pkgs; [
              pkg-config
              llvmPackages.libclang
              llvmPackages.clang
              wrapGAppsHook4
            ];

            buildInputs = with pkgs; [
              libX11
              libXcursor
              libXrandr
              libXi
              libXext
              libXft
              libXinerama
              libXrender
              libXfixes
              wayland
              libxkbcommon
              glibc.dev
              libGL
            ];

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            BINDGEN_EXTRA_CLANG_ARGS =
              "-isystem ${pkgs.glibc.dev}/include";

            postInstall = ''
              install -Dm644 packaging/linux/dev.keyframe.Keyframe.desktop $out/share/applications/dev.keyframe.Keyframe.desktop
              install -Dm644 packaging/keyframe.svg $out/share/icons/hicolor/scalable/apps/dev.keyframe.Keyframe.svg
            '';
          };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
              rust-analyzer
              clippy
              rustfmt
              cargo
              llvmPackages.libclang
              llvmPackages.clang
              libX11
              libXcursor
              libXrandr
              libXi
              libXext
              libXft
              libXinerama
              libXrender
              libXfixes
              wayland
              libxkbcommon
              glibc.dev
              libGL
            ];

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            BINDGEN_EXTRA_CLANG_ARGS =
              "-isystem ${pkgs.glibc.dev}/include";

            shellHook = ''
              export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${
                pkgs.lib.makeLibraryPath [
                  pkgs.wayland
                  pkgs.libxkbcommon
                  pkgs.libGL
                ]
              }:/run/opengl-driver/lib"
            '';
          };
        }
      );
    };
}
