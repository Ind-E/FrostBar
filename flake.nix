{
  description = "frostbar";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
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

      rustToolchain = pkgs: pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

      packageNativeBuildInputs =
        pkgs: with pkgs; [
          lld
          pkg-config
          rustPlatform.bindgenHook
        ];

      packageBuildInputs =
        pkgs: with pkgs; [
          openssl
          alsa-lib
          pipewire
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          vulkan-loader
          libxkbcommon
          fontconfig
          wayland
          rust-jemalloc-sys
          expat
          freetype
          freetype.dev
          libGL
        ];

      devOnlyInputs =
        pkgs: with pkgs; [
          lldb
          rust-bin.nightly.latest.rust-analyzer
          python313Packages.mkdocs-material
          # for tracy
          stdenv.cc.cc
        ];

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
            platformRustFlagsEnv = lib.optionalString pkgs.stdenv.isLinux "-Clink-arg=-Wl,--no-rosegment -Clink-arg=-lwayland-client";
          in
          pkgs.mkShell rec {
            nativeBuildInputs = [
              (rustToolchain pkgs)
            ]
            ++ (packageNativeBuildInputs pkgs)
            ++ (devOnlyInputs pkgs);
            buildInputs = packageBuildInputs pkgs;

            shellHook = ''
              export RUST_BACKTRACE="1"
              export RUSTFLAGS="''${RUSTFLAGS:-""} ${commonRustFlagsEnv} ${platformRustFlagsEnv}"
            '';

            LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          };
      }) pkgsFor;

      overlays = {
        frostbar = final: prev: {
          frostbar =
            let
              toolchain = rustToolchain final;
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

              nativeBuildInputs = packageNativeBuildInputs final;
              buildInputs = packageBuildInputs final;

              buildType = "release";
              strictDeps = true;
              doCheck = false;
            };
        };
        default = self.overlays.frostbar;
      };
    };
}
