{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustup
            pkg-config
            elfutils
            zlib
            # Unwrapped clang for BPF compilation, the NixOS wrapper injects
            # flags (e.g. -fzero-call-used-regs) that are invalid for -target bpf.
            llvmPackages_latest.clang-unwrapped
          ];
        };
      }
    );
}
