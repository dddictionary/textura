{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [ pkgs.llvmPackages pkgs.clang pkgs.cargo pkgs.rustc pkgs.opencv ];

  shellHook = ''
    export LLVM_CONFIG_PATH=${pkgs.llvmPackages}/bin/llvm-config
    export LIBCLANG_PATH=${pkgs.llvmPackages}/lib
  '';
}
