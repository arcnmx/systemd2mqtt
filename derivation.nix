{ rustPlatform
, nix-gitignore
, buildType ? "release"
, openssl, pkg-config
, paho-mqtt-c
, lib
, cargoLock ? {
  lockFile = ./Cargo.lock;
  outputHashes."hass-mqtt-discovery-0.1.0" = "sha256-h9q1yH6pgERAsMzP/Ha0nQsCjKyHnI+Zox8P85tl1Fs=";
}
, _arg'systemd2mqtt ? nix-gitignore.gitignoreSourcePure [ ./.gitignore ''
  /.github
  /.git
  *.nix
'' ] ./.
}: with lib; let
  cargoToml = importTOML ./Cargo.toml;
in rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = _arg'systemd2mqtt;
  inherit cargoLock;
  buildInputs = [
    paho-mqtt-c
    openssl
  ];
  nativeBuildInputs = [
    pkg-config
  ];
  inherit buildType;

  doCheck = false;

  meta = {
    platforms = platforms.unix;
  };
}
