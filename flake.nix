{
  description = "Zako3 Flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          name = "zako3";
          nativeBuildInputs = with pkgs; [
            pkg-config
            autoconf
            gnumake
            heaptrack
          ];
          buildInputs = with pkgs; [
            openssl
            libopus
            pkg-config
          ];
          hardeningDisable = [ "fortify" ];
        };
      }
    );
}
