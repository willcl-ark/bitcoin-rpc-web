{
  description = "DrahtBot";

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
            filter = path: type:
              (craneLib.filterCargoSources path type)
              || (builtins.match ".*/web/.*" path != null)
              || (builtins.match ".*/assets/.*" path != null)
              || (builtins.match ".*/tunes/.*" path != null);
          };
          commonArgs = {
            inherit src;
            nativeBuildInputs =
              [ pkgs.pkg-config ]
              ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
                pkgs.wrapGAppsHook3
              ];
            buildInputs =
              [ pkgs.openssl pkgs.zeromq ]
              ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
                pkgs.webkitgtk_4_1
                pkgs.gtk3
                pkgs.glib
                pkgs.libsoup_3
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

      devShells = forEachSupportedSystem (pkgs: {
        default = (crane.mkLib pkgs).devShell {
          packages = with pkgs; [
            rust-analyzer
            openssl
            pkg-config
            cargo-deny
            cargo-edit
            cargo-watch
            zeromq
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            webkitgtk_4_1
            gtk3
            glib
            libsoup_3
            alsa-lib
          ];
          env = {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        };
      });
    };
}
