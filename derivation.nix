{ rustPlatform
, nix-gitignore
, buildType ? "release"
, openssl, pkg-config
, nixpkgs-paho ? let
  lockData = builtins.fromJSON (builtins.readFile ./flake.lock);
  sourceInfo = lockData.nodes.nixpkgs-paho.locked;
in fetchTarball {
  url = "https://github.com/${sourceInfo.owner}/${sourceInfo.repo}/archive/${sourceInfo.rev}.tar.gz";
  sha256 = sourceInfo.narHash;
}
, callPackage, paho-mqtt-c ? callPackage (nixpkgs-paho + "/pkgs/development/libraries/paho-mqtt-c") { }
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
  cargoSha256 = "sha256-UQ5V9No7iHoKYCzQPC8cKlANChq6YxcMmrnA6HSHHSo=";
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
