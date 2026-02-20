{
  description = "Bitcoin RPC Web Dashboard";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f: nixpkgs.lib.genAttrs supportedSystems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forEachSupportedSystem (
        pkgs:
        let
          craneLib = crane.mkLib pkgs;
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              (craneLib.filterCargoSources path type)
              || (builtins.match ".*/web/.*" path != null)
              || (builtins.match ".*/assets/.*" path != null)
              || (builtins.match ".*/tunes/.*" path != null);
          };
          commonArgs = {
            inherit src;
            nativeBuildInputs = [
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.openssl
              pkgs.zeromq
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.expat
              pkgs.fontconfig
              pkgs.freetype
              pkgs.freetype.dev
              pkgs.wayland
              pkgs.libxkbcommon
              # Uncomment for X11 fallback support:
              # pkgs.xorg.libX11
              # pkgs.xorg.libXcursor
              # pkgs.xorg.libXi
              # pkgs.xorg.libXrandr
              pkgs.vulkan-loader
              # Uncomment if you want non-Vulkan GL path support:
              # pkgs.libGL
              pkgs.alsa-lib
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        in
        {
          default = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
        }
      );

      formatter = forEachSupportedSystem (pkgs: pkgs.nixfmt-tree);

      devShells = forEachSupportedSystem (pkgs: {
        default = (crane.mkLib pkgs).devShell {
          packages =
            with pkgs;
            [
              cargo-deny
              cargo-edit
              cargo-watch
              openssl
              pkg-config
              rust-analyzer
              zeromq
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              # GPU backend
              vulkan-loader
              # libGL

              # Window system
              wayland
              libxkbcommon
              xkeyboard_config
              # xorg.libX11
              # xorg.libXcursor
              # xorg.libXi
              alsa-lib
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];
          env = {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          }
          // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
            XKB_CONFIG_ROOT = "${pkgs.xkeyboard_config}/share/X11/xkb";
            RUSTFLAGS = "-C link-arg=-Wl,-rpath,${
              pkgs.lib.makeLibraryPath [
                pkgs.alsa-lib
                # GPU backend
                pkgs.vulkan-loader
                # pkgs.libGL

                # Window system
                pkgs.wayland
                pkgs.libxkbcommon
                # xorg.libX11
                # xorg.libXcursor
                # xorg.libXi
              ]
            }";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib
              # GPU backend
              pkgs.vulkan-loader
              # pkgs.libGl

              # Window system
              pkgs.wayland
              pkgs.libxkbcommon
              # xorg.libX11
              # xorg.libXcursor
              # xorg.libXi
            ];
          };
        };
      });
    };
}
