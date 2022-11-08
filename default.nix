let
  sources = import ./nix/sources.nix;
  nixpkgs-mozilla = import sources.nixpkgs-mozilla;
  pkgs = import sources.nixpkgs {
    overlays =
      [
        nixpkgs-mozilla
        (self: super:
          let chan = self.rustChannelOf { date = "2022-11-05"; channel = "nightly"; };
          in {
            rustc = chan.rust;
            cargo = chan.rust;
          }
        )
      ];
  };
  lib = pkgs.lib;
  naersk = pkgs.callPackage sources.naersk {};
  merged-openssl = pkgs.symlinkJoin { name = "merged-openssl"; paths = [ pkgs.openssl.out pkgs.openssl.dev ]; };
in
naersk.buildPackage {
  root = lib.sourceFilesBySuffices ./. [".rs" ".toml" ".lock" ".sql" ".css" ".js" ".hbs"];
  buildInputs = with pkgs; [ openssl pkgconfig clang llvm llvmPackages.libclang zlib sqlite ];
  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang}/lib";
  OPENSSL_DIR = "${merged-openssl}";
  preInstall = ''
    mkdir -p $out/share
    cp -r ${./multisig-service/static} $out/share/static
    cp -r ${./multisig-service/templates} $out/share/templates
  '';
}
