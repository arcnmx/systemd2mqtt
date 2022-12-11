{
  description = "expose systemd services to mqtt";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
    rust = {
      url = "github:arcnmx/nixexprs-rust";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { flakelib, self, nixpkgs, rust, ... }@inputs: let
    nixlib = nixpkgs.lib;
    impure = builtins ? currentSystem;
    inherit (nixlib)
      filter optional
      hasSuffix
    ;
  in flakelib {
    systems = filter (hasSuffix "-linux") rust.lib.systems;
    inherit inputs;
    config = {
      name = "systemd2mqtt";
    };
    packages = {
      systemd2mqtt = {
        __functor = _: import ./derivation.nix;
        fl'config.args = {
          crate.fallback = self.lib.crate;
        };
      };
      default = { systemd2mqtt }: systemd2mqtt;
    };
    checks = {
      rustfmt = { rust'builders, source }: rust'builders.check-rustfmt-unstable {
        src = source;
      };
      test = { rustPlatform, source, systemd2mqtt }: rustPlatform.buildRustPackage {
        pname = self.lib.crate.package.name;
        inherit (self.lib.crate) cargoLock version;
        inherit (systemd2mqtt) buildInputs nativeBuildInputs;
        buildNoDefaultFeatures = systemd2mqtt.cargoBuildNoDefaultFeatures;
        buildFeatures = systemd2mqtt.cargoBuildFeatures;
        checkNoDefaultFeatures = systemd2mqtt.cargoCheckNoDefaultFeatures;
        checkFeatures = systemd2mqtt.cargoCheckFeatures;
        src = source;
        buildType = "debug";
        meta.name = "cargo test";
      };
    };
    devShells = {
      plain = {
        mkShell, writeShellScriptBin, hostPlatform, lib
      , enableRust ? true, cargo
      , rustTools ? [ ]
      , systemd2mqtt
      }: mkShell {
        RUST_LOG = "systemd2mqtt=debug";
        allowBroken = true;
        inherit rustTools;
        inherit (systemd2mqtt) buildInputs;
        nativeBuildInputs = systemd2mqtt.nativeBuildInputs ++ optional enableRust cargo ++ [
          (writeShellScriptBin "generate" ''nix run .#generate "$@"'')
        ];
      };
      stable = { rust'stable, outputs'devShells'plain }: outputs'devShells'plain.override {
        inherit (rust'stable) mkShell;
        enableRust = false;
      };
      dev = { rust'unstable, outputs'devShells'plain }: outputs'devShells'plain.override {
        inherit (rust'unstable) mkShell;
        enableRust = false;
        rustTools = [ "rust-analyzer" ];
      };
      default = { outputs'devShells }: outputs'devShells.plain;
    };
    nixosModules = let
      inherit (flakelib.lib.Std.Flake) Outputs;
    in {
      systemd2mqtt = Outputs.WrapModule ./nixos.nix;
      default = self.nixosModules.systemd2mqtt;
    };
    overlays = let
      inherit (flakelib.lib.Std.Flake) Outputs;
    in {
      systemd2mqtt = Outputs.WrapOverlay ./overlay.nix;
      default = self.overlays.systemd2mqtt;
    };
    legacyPackages = { callPackageSet }: callPackageSet {
      source = { rust'builders }: rust'builders.wrapSource self.lib.crate.src;

      generate = { rust'builders, outputHashes }: rust'builders.generateFiles {
        paths = {
          "lock.nix" = outputHashes;
        };
      };
      outputHashes = { rust'builders }: rust'builders.cargoOutputHashes {
        inherit (self.lib) crate;
      };
    } { };
    lib = {
      crate = rust.lib.importCargo {
        path = ./Cargo.toml;
        inherit (import ./lock.nix) outputHashes;
      };
      inherit (self.lib.crate.package) version;
    };
  };
}
