{
  description = "expose systemd services to mqtt";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
    nixpkgs-paho = {
      # https://github.com/NixOS/nixpkgs/pull/194375
      url = "github:NixOS/nixpkgs/6925ec14c9d3e9bce426a74a132f1ed1211cbf9a";
      flake = false;
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
        nixpkgs-paho.fallback = inputs.nixpkgs-paho.outPath;
      };
    };
    defaultPackage = "systemd2mqtt";
  };
}
