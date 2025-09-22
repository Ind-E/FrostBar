{
  description = "frostbar";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = lib.genAttrs systems;
      pkgsFor = eachSystem (
        system:
        import nixpkgs {
          localSystem.system = system;
          overlays = [
            (import rust-overlay)
            self.overlays.frostbar
          ];
        }
      );
    in
    {
      packages = eachSystem (system: {
        inherit (pkgsFor.${system}) frostbar;

        default = self.packages.${system}.frostbar;
      });
      checks = lib.mapAttrs (system: pkgs: {
        inherit (self.packages.${system}) frostbar;
      }) pkgsFor;

      devShells = lib.mapAttrs (system: pkgs: {
        default =
          let
            commonRustFlagsEnv = "-C link-arg=-fuse-ld=lld -C target-cpu=native --cfg tokio_unstable";
            platformRustFlagsEnv = lib.optionalString pkgs.stdenv.isLinux "-Clink-arg=-Wl,--no-rosegment";
          in
          pkgs.mkShell rec {
            inputsFrom = [ self.checks.${system}.frostbar ];
            nativeBuildInputs = with pkgs; [
              lld
              lldb
              rust-bin.nightly.latest.rust-analyzer

              pkg-config

              alsa-lib
              pipewire
              expat
              fontconfig
              freetype
              freetype.dev
              libGL
              libxkbcommon

              stdenv.cc.cc.lib

              wayland
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
              vulkan-loader

              python313Packages.mkdocs-material
            ];

            shellHook = ''
              export RUST_BACKTRACE="1"
              export RUSTFLAGS="''${RUSTFLAGS:-""} ${commonRustFlagsEnv} ${platformRustFlagsEnv}"
            '';

            LD_LIBRARY_PATH = "${lib.makeLibraryPath nativeBuildInputs}";

          };
      }) pkgsFor;

      overlays = {
        frostbar = final: prev: {
          frostbar =
            let
              toolchain = final.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
              rustPlatform = final.makeRustPlatform {
                cargo = toolchain;
                rustc = toolchain;
              };
            in
            rustPlatform.buildRustPackage {
              pname = with builtins; (fromTOML (readFile ./Cargo.toml)).package.name;
              version = with builtins; (fromTOML (readFile ./Cargo.toml)).package.version;
              src = self;
              cargoLock = {
                allowBuiltinFetchGit = true;
                lockFile = ./Cargo.lock;
              };

              buildType = "release";
              strictDeps = true;
              doCheck = false;
            };
        };

        default = self.overlays.frostbar;
      };

    };
}
