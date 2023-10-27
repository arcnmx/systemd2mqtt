let
  self = import ./. { pkgs = null; system = null; };
in {
  rustPlatform
, buildType ? "release"
, openssl, pkg-config
, paho-mqtt-c
, lib
, cargoLock ? crate.cargoLock
, source ? crate.src
, crate ? self.lib.crate
}: with lib; rustPlatform.buildRustPackage {
  pname = crate.name;
  inherit (crate) version;

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
    mainProgram = "systemd2mqtt";
  };
}
