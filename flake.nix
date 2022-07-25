{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages."${system}";
        naersk-lib = naersk.lib."${system}";
      in rec {
        # `nix build`
        packages.wifi-prometheus-exporter = naersk-lib.buildPackage {
          pname = "wifi-prometheus-exporter";
          root = ./.;

          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs = [pkgs.openssl];
        };
        defaultPackage = packages.wifi-prometheus-exporter;
        defaultApp = packages.wifi-prometheus-exporter;

        # `nix develop`
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [rustc cargo bacon cargo-edit cargo-outdated pkgs.openssl pkgs.pkg-config];
        };
      }
    )
    // {
      nixosModule = {
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
          };

          config = mkIf cfg.enable {
            systemd.services."wifi-prometheus-exporter" = let
              pkg = self.defaultPackage.${pkgs.system};
            in {
              wantedBy = ["multi-user.target"];
              script = "${pkg}/bin/wifi-prometheus-exporter";
              environment = {
                PORT = toString cfg.port;
                ADDR = cfg.sshAddress;
                KEYFILE = cfg.sshKeyFile;
                PUBFILE = cfg.sshPubKeyFile;
                INTERFACES = concatStringsSep " " cfg.interfaces;
              };

              serviceConfig = {
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
        };
    };
}
