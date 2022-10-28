{
  description = "expose systemd services to mqtt";
  inputs = {
    flakelib.url = "github:flakelib/fl";
    nixpkgs = { };
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
  };
}
