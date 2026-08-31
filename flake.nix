{
  description = "Kog cross-platform music player";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-intel-darwin.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    inputs@{
      nixpkgs,
      nixpkgs-intel-darwin,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        nixpkgsForSystem = if system == "x86_64-darwin" then nixpkgs-intel-darwin else nixpkgs;
        pkgs = import nixpkgsForSystem { inherit system; };
        qtModules =
          (with pkgs.qt6; [
            qtbase
            qtdeclarative
            qttools
          ])
          ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            pkgs.qt6.qtwayland
          ];
        qtEnv = pkgs.qt6.env "kog-qt-env" qtModules;
      in
      {
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              cargo
              clang
              clippy
              cmake
              ninja
              pkg-config
              rust-analyzer
              rustc
              rustfmt
            ])
            ++ qtModules;

          QMAKE = "${qtEnv}/bin/qmake";
          FLAKE_INPUTS = builtins.concatStringsSep ":" (
            map (input: input.outPath) (builtins.attrValues (builtins.removeAttrs inputs [ "self" ]))
          );

          shellHook = ''
            export PATH="${qtEnv}/bin:${qtEnv}/libexec:$PATH"
            export QMAKE="${qtEnv}/bin/qmake"
            export QT_INCLUDE_PATH="${qtEnv}/include"
            export QT_LIBEXEC_PATH="${qtEnv}/libexec"
          '';
        };

        formatter = pkgs.nixfmt;
      }
    );
}
