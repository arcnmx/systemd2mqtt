{ rustPlatform
, nix-gitignore
, buildType ? "release"
, openssl, pkg-config
, paho-mqtt-c
, lib
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
  cargoSha256 = "sha256-SjrRfetGuv//lSLJyBhwMGOExJ6R1JVRsTRbqp4VF88=";
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
