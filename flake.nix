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
  outputs = { flakelib, ... }@inputs: flakelib {
    inherit inputs;
    config = {
      name = "systemd2mqtt";
    };
    packages.systemd2mqtt = {
      __functor = _: import ./derivation.nix;
      fl'config.args = {
        _arg'systemd2mqtt.fallback = inputs.self.outPath;
      };
    };
    defaultPackage = "systemd2mqtt";
    devShells = {
      plain = { mkShell, systemd2mqtt, cargo, enableRust ? true, lib }: mkShell {
        inherit (systemd2mqtt) buildInputs;
        nativeBuildInputs = systemd2mqtt.nativeBuildInputs ++ lib.optional enableRust cargo;
      };
      stable = { outputs'devShells'plain, rust'stable }: outputs'devShells'plain.override {
        mkShell = args: rust'stable.mkShell (args // {
          rustTools = [ "rust-analyzer" "rust-src" ];
        });
        enableRust = false;
      };
      default = { outputs'devShells'plain }: outputs'devShells'plain;
    };
  };
}
