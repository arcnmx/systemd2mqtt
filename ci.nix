{ config, channels, pkgs, env, lib, ... }: with pkgs; with lib; let
  cargo = name: command: ci.command {
    name = "cargo-${name}";
    command = "cargo " + command;
    impure = true;
    PKG_CONFIG_PATH = makeSearchPath "lib/pkgconfig" systemd2mqtt.buildInputs;
    "NIX_LDFLAGS_${replaceStrings [ "-" ] [ "_" ] hostPlatform.config}" = map (i: "-L${i}/lib") systemd2mqtt.buildInputs;
  };
  systemd2mqtt = callPackage ./derivation.nix {
    buildType = "debug";
  };
in {
  config = {
    name = "systemd2mqtt";
    ci.gh-actions = {
      enable = true;
      emit = true;
    };
    cache.cachix.arc.enable = true;
    channels = {
      nixpkgs = mkIf (env.platform != "impure") "22.11";
      rust = "master";
    };
    environment = {
      test = {
        inherit (pkgs) cargo pkg-config;
        inherit (stdenv) cc;
      };
    };
    tasks = {
      test.inputs = cargo "test" "test";
      build.inputs = systemd2mqtt;
    };
  };
}
