{ config
, lib
, pkgs
, ...
}:
with lib; let
  cfg = config.services.prometheus.exporters.wifi;
  format = pkgs.formats.toml { };
  configFile = format.generate "wifi-prometheus-exporter-config.toml" {
    ssh = {
      inherit (cfg.ssh) address user;
      key_file = "$CREDENTIALS_DIRECTORY/ssh_key";
      pubkey_file = "$CREDENTIALS_DIRECTORY/ssh_pub_key";
    };
    exporter = {
      inherit (cfg) interfaces port;
    };
    mqtt = {
      inherit (cfg.mqtt) hostname port username;
      password_file = "$CREDENTIALS_DIRECTORY/mqtt_password";
    };
  };

in
{
  options.services.prometheus.exporters.wifi = {
    enable = mkEnableOption "WiFi prometheus exporter";

    ssh = mkOption {
      type = types.submodule {
        options = {
          address = mkOption {
            type = types.str;
            description = "ssh address of the access point";
          };
          user = mkOption {
            type = types.str;
            description = "ssh user";
          };
          keyFile = mkOption {
            type = types.str;
            description = "path to ssh key file";
          };
          pubKeyFile = mkOption {
            type = types.str;
            description = "path to ssh public key";
          };
        };
      };
    };

    mqtt = mkOption {
      type = types.submodule {
        options = {
          hostname = mkOption {
            type = types.str;
            description = "mqtt server hostname";
          };
          port = mkOption {
            type = types.port;
            description = "mqtt server port";
            default = 1883;
          };
          username = mkOption {
            type = types.str;
            description = "mqtt username";
          };
          passwordFile = mkOption {
            type = types.str;
            description = "path containing the mqtt password";
          };
        };
      };
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

    package = mkOption {
      type = types.package;
      defaultText = literalExpression "pkgs.dispenser";
      description = "package to use";
    };

    log = mkOption {
      type = types.str;
      description = "log level";
      default = "info";
    };
  };

  config = mkIf cfg.enable {
    systemd.services."wifi-prometheus-exporter" = {
      wantedBy = [ "multi-user.target" ];
      environment = {
        RUST_LOG = cfg.log;
      };

      serviceConfig = {
        ExecStart = "${pkgs.wifi-prometheus-exporter}/bin/wifi-prometheus-exporter ${configFile}";
        LoadCredential = [
          "ssh_key:${cfg.ssh.keyFile}"
          "ssh_pub_key:${cfg.ssh.pubKeyFile}"
          "mqtt_password:${cfg.mqtt.passwordFile}"
        ];
        Restart = "on-failure";
        RestartSec = "30s";
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
        SystemCallFilter = [ "@system-service" "~@resources" "~@privileged" ];
        IPAddressDeny = "any";
        IPAddressAllow = [ "192.168.0.0/16" "localhost" ];
        PrivateUsers = true;
        ProcSubset = "pid";
      };
    };
  };
}
