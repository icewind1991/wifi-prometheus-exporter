{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-24.05";
    flakelight = {
      url = "github:nix-community/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    mill-scale = {
      url = "github:icewind1991/mill-scale";
      inputs.flakelight.follows = "flakelight";
    };
  };
  outputs = { mill-scale, ... }: mill-scale ./. {
    packages.wifi-prometheus-exporter = import ./package.nix;

    nixosModules = { outputs, ... }: {
      default =
        { pkgs
        , config
        , lib
        , ...
        }: {
          imports = [ ./module.nix ];
          config = {
            nixpkgs.overlays = [ outputs.overlays.default ];
            services.prometheus.exporters.wifi.package = lib.mkDefault pkgs.wifi-prometheus-exporter;
          };
        };
    };
  };
}
