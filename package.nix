{ rustPlatform
, openssl
, pkg-config
, lib
,
}:
let
  inherit (lib.sources) sourceByRegex;
  src = sourceByRegex ./. [ "Cargo.*" "(src)(/.*)?" ];
in
rustPlatform.buildRustPackage rec {
  pname = "wifi-prometheus-exporter";
  version = "0.1.0";

  inherit src;

  buildInputs = [
    openssl
  ];

  nativeBuildInputs = [
    pkg-config
  ];

  doCheck = false;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };
}
