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
        # Nixpkgs' libarchive omits liblz4 and otherwise launches an external
        # lz4 executable. Keep archive extraction inside the shared backend.
        kogLibarchive = pkgs.libarchive.overrideAttrs (previous: {
          buildInputs = (previous.buildInputs or [ ]) ++ [ pkgs.lz4 ];
          configureFlags = (previous.configureFlags or [ ]) ++ [ "--with-lz4" ];
        });
        # Read as a string so the store path only changes with the lock's
        # content, not with every source tree or commit.
        KOG_CARGO_VENDOR = (crane.mkLib pkgs).vendorCargoDeps {
          cargoLockContents = builtins.readFile ./Cargo.lock;
        };
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        kogLibraries = qtModules ++ [ kogFfmpeg kogLibarchive pkgs.alsa-lib pkgs.zlib pkgs.libxcb-cursor ];
        # What the package's setup hooks compute, propagated libraries
        # included, so the dev loop can use exactly the same search path.
        KOG_PKG_CONFIG_PATH = pkgs.runCommand "kog-pkg-config-path"
          {
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = kogLibraries;
          }
          ''
            printf %s "$PKG_CONFIG_PATH" > $out
          '';
        linuxRuntimeLibraries = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.libxcb-cursor
        ];
      in
      {
        packages = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux (
          let
            craneLib = crane.mkLib pkgs;
            # The web frontend is wasm-only and lives outside the desktop
            # workspace, so it gets its own derivation and is embedded into
            # the server at compile time.
            kogWeb = craneLib.buildPackage {
              pname = "kog-web";
              version = (builtins.fromTOML (builtins.readFile ./crates/kog-web/Cargo.toml)).package.version;
              # Preserve the repository layout: the frontend imports the
              # shared playback policy and the MML/inspection crate (whose
              # guide chapters it embeds), Qt's format icons and the
              # tracker's pixel font.
              src = pkgs.lib.fileset.toSource {
                root = ./.;
                fileset = pkgs.lib.fileset.unions [
                  ./crates/kog-web/Cargo.toml
                  ./crates/kog-web/Cargo.lock
                  ./crates/kog-web/src
                  ./crates/kog-web/index.html
                  ./crates/kog-web/style.css
                  ./crates/kog-web/manifest.webmanifest
                  ./crates/kog-web/icons
                  ./crates/kog-playback-policy
                  ./crates/kog-inspection
                  ./docs/mml-guide
                  ./qml/icons
                  ./qml/fonts
                ];
              };
              postUnpack = ''sourceRoot="$sourceRoot/crates/kog-web"'';
              cargoVendorDir = craneLib.vendorCargoDeps {
                cargoLock = ./crates/kog-web/Cargo.lock;
              };
              cargoArtifacts = null;
              doCheck = false;
              nativeBuildInputs = [ pkgs.wasm-bindgen-cli pkgs.lld ];
              CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "wasm-ld";
              cargoBuildCommand = "cargoWithProfile build --target wasm32-unknown-unknown";
              installPhaseCommand = ''
                mkdir -p $out
                wasm-bindgen --target web --no-typescript --out-dir $out \
                  target/wasm32-unknown-unknown/release/kog_web.wasm
                cp index.html style.css manifest.webmanifest $out/
                cp -r icons $out/icons
                mkdir -p $out/fonts
                cp ../../qml/fonts/spleen-6x12.otf ../../qml/fonts/Ubuntu-*.ttf $out/fonts/
                # Match build.sh: the server serves precompressed assets on
                # mobile, while rewriting index.html and kog_web.js itself.
                find "$out" -type f ! -name '*.gz' ! -name 'index.html' ! -name 'kog_web.js' -exec sh -c \
                  'gzip -9 -c "$1" > "$1.gz"' _ {} \;
              '';
            };
            # One compile for dev and the system: this builds in the target
            # directory scripts/dev.sh uses, so it reuses what the dev loop
            # compiled. Falls back to a throwaway directory when the sandbox
            # doesn't expose the cache.
            compiled = pkgs.stdenv.mkDerivation {
              pname = "kog";
              version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
              src = ./.;
              nativeBuildInputs = [
                pkgs.cargo
                pkgs.rustc
                pkgs.cmake
                pkgs.ninja
                pkgs.pkg-config
                pkgs.clang
                pkgs.mold
                pkgs.rsync
                pkgs.qt6.wrapQtAppsHook
              ];
              buildInputs = kogLibraries;
              inherit KOG_CARGO_VENDOR KOG_PKG_CONFIG_PATH LIBCLANG_PATH;
              QMAKE = "${qtEnv}/bin/qmake";
              # Cargo invokes CMake/Ninja for decoder libraries; they must not
              # replace Cargo's top-level configure/build/install phases.
              dontUseCmakeConfigure = true;
              dontUseNinjaBuild = true;
              dontUseNinjaCheck = true;
              dontUseNinjaInstall = true;
              dontConfigure = true;
              # Rewriting native/**/config.guess would mark native/ as changed.
              dontUpdateAutotoolsGnuConfigScripts = true;
              buildPhase = ''
                runHook preBuild
                export PATH="${qtEnv}/bin:${qtEnv}/libexec:$PATH"
                # Qt's setup hook can replace QMAKE with qtbase's split output.
                # CXX-Qt needs the combined installation's QML .prl metadata.
                export QMAKE="${qtEnv}/bin/qmake"
                export QT_INCLUDE_PATH="${qtEnv}/include"
                export QT_LIBEXEC_PATH="${qtEnv}/libexec"
                # The crate falls back to a committed placeholder page.
                rm -rf crates/kog-server/web
                mkdir -p crates/kog-server/web
                cp -r ${kogWeb}/. crates/kog-server/web/
                chmod -R u+w crates/kog-server/web
                # A build script rerun by a different user can't reuse the
                # other's output directory, so build privately if this fails.
                export KOG_WORK_DIR=/var/tmp/cargo-nix/kog
                if ! { [ -w /var/tmp/cargo-nix ] && bash scripts/shared-build.sh . build --workspace --bins; }; then
                  export KOG_WORK_DIR=$TMPDIR/cargo-nix
                  bash scripts/shared-build.sh . build --workspace --bins
                fi
                mkdir -p bin
                cp "$KOG_WORK_DIR"/target/debug/{kog,kog-tui,kog-server} bin/
                runHook postBuild
              '';
              installPhase = ''
                runHook preInstall
                install -Dm755 -t "$out/bin" bin/*
                install -Dm644 packaging/linux/org.kog.player.desktop "$out/share/applications/org.kog.player.desktop"
                install -Dm644 qml/icons/kog.svg "$out/share/icons/hicolor/scalable/apps/org.kog.player.svg"
                install -Dm644 packaging/linux/org.kog.player.metainfo.xml "$out/share/metainfo/org.kog.player.metainfo.xml"
                runHook postInstall
              '';
            };
          in
          {
            # Exposed so the frontend can be built (and inspected) on its own:
            # release jobs build it, then the package build embeds it.
            kog-web = kogWeb;
            inherit compiled;
            # The revision lives here rather than in the compiled package, so
            # a commit that doesn't change the source reuses the compile.
            default = pkgs.symlinkJoin {
              name = "kog-${compiled.version}";
              paths = [ compiled ];
              nativeBuildInputs = [ pkgs.makeWrapper ];
              postBuild = ''
                wrapProgram $out/bin/kog \
                  --set KOG_BUILD_REV "${inputs.self.shortRev or inputs.self.dirtyShortRev or "unknown"}"
              '';
              meta = {
                description = "Format-comprehensive local music player";
                homepage = "https://github.com/benwbooth/Kog";
                license = pkgs.lib.licenses.gpl3Plus;
                platforms = pkgs.lib.platforms.linux;
                mainProgram = "kog";
              };
            };
          }
        );
        apps = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          default = { type = "app"; program = "${inputs.self.packages.${system}.default}/bin/kog"; };
        };
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              bacon
              cargo
              cargo-watch
              clang
              clippy
              cmake
              ninja
              nodejs
              pkg-config
              lld
              mold
              rsync
              rust-analyzer
              rustc
              rustfmt
              wasm-bindgen-cli
              watchexec
              zlib
            ])
            ++ [ kogFfmpeg kogLibarchive ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.alsa-lib
              pkgs.libxcb-cursor
            ]
            ++ qtModules;

          QMAKE = "${qtEnv}/bin/qmake";
          inherit KOG_CARGO_VENDOR KOG_PKG_CONFIG_PATH LIBCLANG_PATH;
          FLAKE_INPUTS = builtins.concatStringsSep ":" (
            map (input: input.outPath) (builtins.attrValues (builtins.removeAttrs inputs [ "self" ]))
          );

          shellHook = ''
            export PATH="${qtEnv}/bin:${qtEnv}/libexec:$PATH"
            # The host environment can carry NIX_CFLAGS_LINK="-L/usr/lib",
            # and on NixOS /usr/lib holds the system's glibc, which can be
            # newer than the shell's; a binary linked against it won't load.
            # Drop those paths so links here only see the shell's glibc.
            NIX_CFLAGS_LINK="$(printf '%s\n' ''${NIX_CFLAGS_LINK:-} | grep -v '^-L/usr/lib' | tr '\n' ' ')"
            export NIX_CFLAGS_LINK
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
