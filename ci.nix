{ config, pkgs, lib, ... }: with pkgs; with lib; let
  inherit (import ./. { inherit pkgs; }) checks packages;
  systemd2mqtt = packages.systemd2mqtt.override {
    buildType = "debug";
  };
in {
  config = {
    name = "systemd2mqtt";
    ci.gh-actions.enable = true;
    cache.cachix = {
      ci.signingKey = "";
      arc.enable = true;
    };
    channels = {
      nixpkgs = "22.11";
    };
    tasks = {
      build.inputs = singleton systemd2mqtt;
    };
  };
}
