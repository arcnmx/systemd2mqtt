let
  self = import ./. { pkgs = null; system = null; };
in { rustPlatform
, nix-gitignore
, buildType ? "release"
, openssl, pkg-config
, paho-mqtt-c
, lib
, cargoLock ? self.lib.crate.cargoLock
, source ? self.lib.crate.src
}: with lib; let
  cargoToml = importTOML ./Cargo.toml;
in rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = source;
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
