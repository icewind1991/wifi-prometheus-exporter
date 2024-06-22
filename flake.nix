{
  inputs = {
    nixpkgs.url = "nixpkgs/release-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [
          (import ./overlay.nix)
        ];
        pkgs = (import nixpkgs) {
          inherit system overlays;
        };
      in rec {
        packages = rec {
          inherit (pkgs) wifi-prometheus-exporter;
          default = wifi-prometheus-exporter;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [rustc cargo bacon cargo-edit cargo-outdated openssl pkg-config clippy];
        };
      }
    )
    // {
      overlays.default = import ./overlay.nix;
      nixosModules.default = {
        pkgs,
        config,
        lib,
        ...
      }: {
        imports = [./module.nix];
        config = lib.mkIf config.services.wifi-prometheus-exporter.enable {
          nixpkgs.overlays = [self.overlays.default];
          services.wifi-prometheus-exporter.package = lib.mkDefault pkgs.wifi-prometheus-exporter;
        };
      };
    };
}
