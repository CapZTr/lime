{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [rust-overlay.overlays.default];
    };
  in {
    devShells.${system}.default = with pkgs;
      pkgs.mkShell {
        buildInputs = [
          cbc
          bzip2
          libz
          pkg-config
          bashInteractive
          cmake
          # cbc
          rust-bin.stable.latest.default
        ];
        LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
      };
  };
}
