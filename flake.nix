{
  description = "Kog cross-platform music player";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-intel-darwin.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    inputs@{
      nixpkgs,
      nixpkgs-intel-darwin,
      flake-utils,
      crane,
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
            qtsvg
            qttools
            qtwebengine
            qtwebchannel
          ])
          ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            pkgs.qt6.qtwayland
            pkgs.kdePackages.plasma-integration
            pkgs.kdePackages.qqc2-desktop-style
            pkgs.kdePackages.breeze-icons
            pkgs.kdePackages.layer-shell-qt
          ];
        qtEnv = pkgs.qt6.env "kog-qt-env" qtModules;
        # Keep a conservative, reproducible LGPL FFmpeg baseline. Kog's
        # GPL-3.0-or-later license also permits compatible GPLv3 FFmpeg builds.
        # Native FFmpeg audio demuxers/decoders remain available here without
        # enabling those additional components.
        kogFfmpeg = pkgs.ffmpeg-headless.override {
          withGPL = false;
          withVersion3 = false;
        };
        linuxRuntimeLibraries = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.libxcb-cursor
        ];
      in
      {
        packages = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux (
          let
            craneLib = crane.mkLib pkgs;
            # Shared build environment for the dependency closure and the
            # final crate: third-party dependencies compile once and stay
            # cached while only Kog itself rebuilds per tag.
            #
            # Version bumps must NOT invalidate that cache: the dependency
            # graph never depends on our own version string, so the deps
            # build sees manifests with the version pinned to a constant
            # while the final package builds the real tree (and reports
            # the real version).
            normalizedSrc = pkgs.runCommand "kog-deps-src"
              {
                nativeBuildInputs = [ pkgs.python3 ];
              }
              ''
                cp -r ${builtins.fetchGit { url = "/home/ben/src/Kog"; rev = "85c6d749bc8549e4c8e6c8ec0438ae8ef59d47f4"; submodules = true; }} $out
                chmod -R u+w $out
                ${pkgs.python3}/bin/python3 - <<'PYEOF'
                import os
                import re
                root = os.environ["out"]
                # Every workspace member manifest: pin the version constant so
                # release bumps cannot invalidate the cached dependency build.
                manifests = [os.path.join(root, "Cargo.toml")]
                crates = os.path.join(root, "crates")
                if os.path.isdir(crates):
                    for member in sorted(os.listdir(crates)):
                        candidate = os.path.join(crates, member, "Cargo.toml")
                        if os.path.isfile(candidate):
                            manifests.append(candidate)
                for manifest in manifests:
                    text = open(manifest).read()
                    head, sep, tail = text.partition("[dependencies]")
                    assert sep, manifest + " has no [dependencies] section"
                    head = re.sub(
                        r'(?m)^version = "[^"]*"$', 'version = "0.0.0"', head, count=1
                    )
                    open(manifest, "w").write(head + sep + tail)
                lock = os.path.join(root, "Cargo.lock")
                text = open(lock).read()
                for name in ("kog", "kog-core", "kog-audio"):
                    text, count = re.subn(
                        r'(\[\[package\]\]\nname = "' + name + r'"\nversion = ")[^"]*(")',
                        r"\g<1>0.0.0\g<2>",
                        text,
                        count=1,
                    )
                    assert count == 1, name + " stanza not found in Cargo.lock"
                open(lock, "w").write(text)
                PYEOF
              '';
            commonArgs = {
              src = builtins.fetchGit { url = "/home/ben/src/Kog"; rev = "85c6d749bc8549e4c8e6c8ec0438ae8ef59d47f4"; submodules = true; };
              nativeBuildInputs = [ pkgs.cmake pkgs.ninja pkgs.pkg-config pkgs.clang pkgs.mold pkgs.qt6.wrapQtAppsHook ];
              # Fast links: the final binary is huge (Qt/C++ in release).
              RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
              # Cargo invokes CMake/Ninja for decoder libraries; they must not
              # replace Cargo's top-level configure/build/install phases.
              dontUseCmakeConfigure = true;
              dontUseNinjaBuild = true;
              dontUseNinjaCheck = true;
              dontUseNinjaInstall = true;
              buildInputs = qtModules ++ [ kogFfmpeg pkgs.libarchive pkgs.alsa-lib pkgs.zlib pkgs.libxcb-cursor ];
              QMAKE = "${qtEnv}/bin/qmake";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              preBuild = ''
                export PATH="${qtEnv}/bin:${qtEnv}/libexec:$PATH"
                # Qt's setup hook can replace QMAKE with qtbase's split output.
                # CXX-Qt needs the combined installation's QML .prl metadata.
                export QMAKE="${qtEnv}/bin/qmake"
                export QT_INCLUDE_PATH="${qtEnv}/include"
                export QT_LIBEXEC_PATH="${qtEnv}/libexec"
              '';
            };
            cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
              src = normalizedSrc;
              # The dummy workspace crane synthesizes for the deps pass does
              # not round-trip every member manifest, so `--locked` would
              # reject its own synthetic lock. Resolution is still pinned by
              # the vendored crate set, so dropping the flag is safe here;
              # the real package build below keeps `--locked`.
              cargoExtraArgs = "";
            });
          in
          {
            default = craneLib.buildPackage (
              commonArgs
              // {
                postInstall = ''
                  for helper in kog-sfm-helper kog-psf-helper kog-psf2-helper kog-2sf-helper kog-snsf-helper kog-syntrax-helper kog-sc55-helper; do
                    helperPath=$(find target -type f -path "*/bin/$helper" -print -quit)
                    test -n "$helperPath" || { echo "Missing decoder helper: $helper" >&2; exit 1; }
                    install -m755 "$helperPath" "$out/bin/$helper"
                  done
                  install -Dm644 packaging/linux/org.kog.player.desktop "$out/share/applications/org.kog.player.desktop"
                  install -Dm644 qml/icons/kog.svg "$out/share/icons/hicolor/scalable/apps/org.kog.player.svg"
                  install -Dm644 packaging/linux/org.kog.player.metainfo.xml "$out/share/metainfo/org.kog.player.metainfo.xml"
                '';
                meta = {
                  description = "Format-comprehensive local music player";
                  homepage = "https://github.com/benwbooth/Kog";
                  license = pkgs.lib.licenses.gpl3Plus;
                  platforms = pkgs.lib.platforms.linux;
                  mainProgram = "kog";
                };
              }
            );
          }
        );
        apps = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          default = { type = "app"; program = "${inputs.self.packages.${system}.default}/bin/kog"; };
        };
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              cargo
              clang
              clippy
              cmake
              libarchive
              ninja
              nodejs
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
