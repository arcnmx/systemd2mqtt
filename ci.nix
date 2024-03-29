{ pkgs, env, lib, ... }: with pkgs; with lib; let
  inherit (import ./. { inherit pkgs; }) checks packages;
  systemd2mqtt = packages.systemd2mqtt.override {
    buildType = "debug";
  };
in {
  config = {
    name = "systemd2mqtt";
    ci.version = "v0.6";
    ci.gh-actions.enable = true;
    cache.cachix = {
      ci.signingKey = "";
      arc.enable = true;
    };
    channels = {
      nixpkgs = mkIf (env.platform != "impure") "23.05";
    };
    tasks = {
      build.inputs = singleton systemd2mqtt;
      rustfmt.inputs = singleton checks.rustfmt;
    };
  };
}
