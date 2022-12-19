let
  self = import ./. { pkgs = null; system = null; };
in {
  rustPlatform
, nix-gitignore
, buildType ? "release"
, openssl, pkg-config
, paho-mqtt-c ? null
, lib
, cargoLock ? crate.cargoLock
, source ? crate.src
, crate ? self.lib.crate
, enablePaho ? paho-mqtt-c != null
, enableTls ? true
}: with lib; rustPlatform.buildRustPackage rec {
  pname = crate.name;
  inherit (crate) version;

  src = source;
  inherit cargoLock;
  buildInputs =
    optional enablePaho paho-mqtt-c
  ++ optional (enablePaho && enableTls) openssl;
  nativeBuildInputs =
    optional enablePaho pkg-config;
  inherit buildType;

  buildNoDefaultFeatures = true;
  buildFeatures =
    optional enablePaho "paho"
  ++ optional enableTls "tls";

  checkNoDefaultFeatures = true;
  checkFeatures = remove "paho" buildFeatures;

  meta = {
    platforms = platforms.unix;
  };
}
