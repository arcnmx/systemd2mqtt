{
  description = "expose systemd services to mqtt";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
    nixpkgs-paho = {
      # https://github.com/NixOS/nixpkgs/pull/166862
      url = "github:NixOS/nixpkgs/bcdaa300fd314e05af4b2f74f34938d60e955ca7";
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
