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
        # Kog is GPL-2.0-only, so do not link the default Nix FFmpeg build:
        # its GPLv3 components make that combined work license-incompatible.
        # Native FFmpeg audio demuxers/decoders remain available in this
        # LGPL-2.1-or-later configuration.
        kogFfmpeg = pkgs.ffmpeg-headless.override {
          withGPL = false;
          withVersion3 = false;
        };
        linuxRuntimeLibraries = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.libxcb-cursor
        ];
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
              zlib
            ])
            ++ [ kogFfmpeg ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.alsa-lib
              pkgs.libxcb-cursor
            ]
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
            export QT_PLUGIN_PATH="${qtEnv}/lib/qt-6/plugins"
            export QML_IMPORT_PATH="${qtEnv}/lib/qt-6/qml"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath linuxRuntimeLibraries}:''${LD_LIBRARY_PATH:-}"
          '';
        };

        formatter = pkgs.nixfmt;
      }
    );
}
