{
  config,
  lib,
  pkgs,
  ...
}:
with lib; let
  cfg = config.services.wifi-prometheus-exporter;
in {
  options.services.wifi-prometheus-exporter = {
    enable = mkEnableOption "WiFi prometheus exporter";

    sshAddress = mkOption {
      type = types.str;
      description = "ssh address of the wifi access point";
    };

    sshKeyFile = mkOption {
      type = types.str;
      description = "path containing the ssh private key";
    };

    sshPubKeyFile = mkOption {
      type = types.str;
      description = "path containing the ssh public key";
    };

    interfaces = mkOption {
      type = types.listOf types.str;
      description = "access point interface to expose";
    };

    port = mkOption {
      type = types.int;
      default = 9010;
      description = "port to listen to";
    };

    mqttSecretFile = mkOption {
      type = types.str;
      description = "path containing MQTT_HOSTNAME, MQTT_USERNAME and MQTT_PASSWORD environment variables";
    };

    package = mkOption {
      type = types.package;
      defaultText = literalExpression "pkgs.dispenser";
      description = "package to use";
    };
  };

  config = mkIf cfg.enable {
    systemd.services."wifi-prometheus-exporter" = {
      wantedBy = ["multi-user.target"];
      environment = {
        PORT = toString cfg.port;
        ADDR = cfg.sshAddress;
        KEYFILE = cfg.sshKeyFile;
        PUBFILE = cfg.sshPubKeyFile;
        INTERFACES = concatStringsSep " " cfg.interfaces;
      };

      serviceConfig = {
        ExecStart = "${pkgs.wifi-prometheus-exporter}/bin/wifi-prometheus-exporter";
        EnvironmentFile = cfg.mqttSecretFile;
        Restart = "on-failure";
        DynamicUser = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        ProtectClock = true;
        CapabilityBoundingSet = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        SystemCallArchitectures = "native";
        ProtectKernelModules = true;
        RestrictNamespaces = true;
        MemoryDenyWriteExecute = true;
        ProtectHostname = true;
        LockPersonality = true;
        ProtectKernelTunables = true;
        RestrictAddressFamilies = "AF_INET AF_INET6";
        RestrictRealtime = true;
        ProtectProc = "noaccess";
        SystemCallFilter = ["@system-service" "~@resources" "~@privileged"];
        IPAddressDeny = "any";
        IPAddressAllow = ["192.168.0.0/16" "localhost"];
        PrivateUsers = true;
        ProcSubset = "pid";
      };
    };
  };
}
