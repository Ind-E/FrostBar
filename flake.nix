{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      systems,
      rust-overlay,
      ...
    }:
    let
      frostbar-package =
        {
          pkgs,
          lib,
          rustPlatform,
          makeWrapper,
        }:
        rustPlatform.buildRustPackage {
          pname = "frostbar";
          inherit ((fromTOML (builtins.readFile ./Cargo.toml)).package) version;

          strictDeps = true;

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./src
              ./Cargo.toml
              ./Cargo.lock
              ./assets
            ];
          };

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustPlatform.bindgenHook
            makeWrapper
          ];

          buildInputs = with pkgs; [
            openssl
            pipewire
          ];

          postInstall = ''
            wrapProgram $out/bin/frostbar \
            --set LD_LIBRARY_PATH ${lib.makeLibraryPath (dlopenLibraries pkgs)}
          '';

          env.RUSTFLAGS = RUSTFLAGS pkgs;

        };

      inherit (nixpkgs) lib;

      dlopenLibraries =
        pkgs: with pkgs; [
          libxkbcommon
          vulkan-loader
          wayland
        ];

      RUSTFLAGS = pkgs: "-C link-arg=-Wl,-rpath,${lib.makeLibraryPath (dlopenLibraries pkgs)}";

      eachSystem = lib.genAttrs (import systems);
      pkgsFor = nixpkgs.legacyPackages;
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = pkgsFor.${system};
          rust-bin = rust-overlay.lib.mkRustBin { } pkgs;
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              (rust-bin.beta.latest.default.override {
                extensions = [
                  "rust-src"
                  "rust-analyzer"
                ];
              })

              pkg-config
              rustPlatform.bindgenHook
            ];

            buildInputs = with pkgs; [
              openssl
              pipewire
            ];

            env.RUSTFLAGS = RUSTFLAGS pkgs;
          };
        }
      );

      packages = eachSystem (
        system:
        let
          pkgs = pkgsFor.${system};
          frostbar = pkgs.callPackage frostbar-package { };
        in
        {
          inherit frostbar;
          default = frostbar;

          frostbar-debug = frostbar.overrideAttrs (
            final: prev: {
              pname = prev.pname + "-debug";

              cargoBuildType = "debug";
              cargoCheckType = final.cargoBuildType;

              dontStrip = true;
            }
          );
        }
      );

      overlays.default = final: _: {
        frostbar = final.callPackage frostbar-package { };
      };
    };
}
