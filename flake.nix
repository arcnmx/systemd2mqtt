{
  description = "expose systemd services to mqtt";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
    rust = {
      url = "github:arcnmx/nixexprs-rust";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    arc = {
      url = "github:arcnmx/nixexprs";
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
      systemd2mqtt = ./derivation.nix;
      default = { systemd2mqtt }: systemd2mqtt;
    };
    checks = {
      rustfmt = { rust'builders, source }: rust'builders.check-rustfmt-unstable {
        src = source;
      };
      test = { rustPlatform, source }: rustPlatform.buildRustPackage {
        pname = self.lib.crate.package.name;
        inherit (self.lib.crate) cargoLock version;
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
      dev = { arc'rustPlatforms'nightly, rust'distChannel, outputs'devShells'plain }: let
        channel = rust'distChannel {
          inherit (arc'rustPlatforms'nightly) channel date manifestPath;
        };
      in outputs'devShells'plain.override {
        inherit (channel) mkShell;
        enableRust = false;
        rustTools = [ "rust-analyzer" ];
      };
      default = { outputs'devShells }: outputs'devShells.plain;
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
