{ pkgs, options, config, lib, utils, inputs'systemd2mqtt, ... }: with lib; let
  cfg = config.services.systemd2mqtt;
  StateDirectory = "systemd2mqtt";
  WorkingDirectory = "/var/lib/${StateDirectory}";
  tos = value:
    if value == true then "true"
    else if value == false then "false"
    else toString value;
  unitModule = { config, name, ... }: {
    options = with types; {
      unit = mkOption {
        type = str;
        default = name;
      };
      settings = mkOption {
        type = attrsOf (oneOf [ bool str ]);
        default = { };
      };
      arg = mkOption {
        type = str;
      };
    };
    config.arg = config.unit + optionalString (config.settings != { }) (
      "?" + concatStringsSep "&" (mapAttrsToList (key: value: "${key}=${tos value}") config.settings)
    );
  };
  coerceUnits = module: with types; coercedTo
    (oneOf [ (listOf (oneOf [ attrs str ])) str ])
    (units: listToAttrs (map (v: nameValuePair v.unit or v (if ! str.check v then v else {})) (toList units)))
    (attrsOf module);
in {
  options.services.systemd2mqtt = with types; {
    enable = mkEnableOption "systemd2mqtt";
    units = mkOption {
      type = coerceUnits (submodule unitModule);
      default = { };
    };
    hostName = mkOption {
      type = nullOr str;
      default = if config.networking.hostName != "" then config.networking.hostName else null;
    };
    logLevel = mkOption {
      type = str;
      default = "warn";
    };
    mqtt = {
      url = mkOption {
        type = nullOr str;
      };
      clientId = mkOption {
        type = str;
        default = "systemd" + optionalString (cfg.hostName != null) "-${cfg.hostName}";
      };
      username = mkOption {
        type = nullOr str;
        default = null;
      };
    };
    createUser = mkOption {
      type = bool;
      default = cfg.user == "systemd2mqtt";
    };
    user = mkOption {
      type = str;
      default = "systemd2mqtt";
    };
    package = mkOption {
      type = package;
      default = pkgs.systemd2mqtt or inputs'systemd2mqtt.package;
    };
    extraArgs = mkOption {
      type = listOf str;
      default = [ ];
    };
  };
  config = mkMerge [
    {
      _module.args.inputs'systemd2mqtt = {
        path = ./.;
        package = pkgs.callPackage ./derivation.nix { };
      };
      services.systemd2mqtt.extraArgs = cli.toGNUCommandLine { } {
        ${if cfg.mqtt.url != null then "mqtt-url" else null} = cfg.mqtt.url;
        client-id = cfg.mqtt.clientId;
        ${if cfg.hostName != null then "hostname" else null} = cfg.hostName;
        unit = mapAttrsToList (_: unit: unit.arg) cfg.units;
        ${if cfg.mqtt.username != null then "mqtt-username" else null} = cfg.mqtt.username;
      };
    }
    (mkIf cfg.enable {
      systemd.services.systemd2mqtt = {
        wantedBy = [ "multi-user.target" ];
        wants = [ "network-online.target" ];
        after = [ "network-online.target" ];
        serviceConfig = {
          Type = "notify";
          inherit WorkingDirectory StateDirectory;
          User = mkDefault cfg.user;
          ExecStart = singleton "${getExe cfg.package} ${utils.escapeSystemdExecArgs cfg.extraArgs}";
          Restart = mkDefault "on-failure";
          Environment = [
            "RUST_LOG=${cfg.logLevel}"
          ];
        };
      };
      users.users.${cfg.user} = mkIf cfg.createUser {
        isSystemUser = mkDefault true;
        home = mkDefault WorkingDirectory;
        createHome = mkDefault true;
        group = mkDefault "nogroup";
      };
      security.polkit = optionalAttrs (options ? security.polkit.users) {
        users.${cfg.user} = {
          systemd = {
            units = mapAttrsToList (_: unit: unit.unit) cfg.units;
          };
        };
      };
    })
  ];
}
