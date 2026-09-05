use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Path::canonicalize returns \\?\-prefixed verbatim paths on Windows, which
/// MSBuild refuses to compile CMake source files from, and raw backslashes
/// break when CMake re-emits a path into generated project code; hand both
/// the plain, forward-slash form.
fn plain_absolute(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text.into_owned()
    };
    PathBuf::from(text.replace('\\', "/"))
}

fn main() {
    build_spessasynth_midi();
    build_mt32emu();
    build_game_music_emu();
    build_sfm_helper();
    let libvgm_output = build_libvgm();
    build_psf_helper();
    build_psf2_helper();
    build_twosf_helper();
    build_snsf_helper();
    build_syntrax_helper();
    build_sc55_helper();
    build_adlmidi();
    build_openmpt();
    build_hivelytracker();
    build_vgmstream();
    build_adplug();
    build_libsidplayfp();
    let mgba_output = build_mgba();
    build_ncsf(&mgba_output);
    build_ffmpeg();

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include("native/opl3w")
        .include("native/opl3w/synthlib_opl3w")
        .files([
            "native/opl3w/kog_opl3w.cpp",
            "native/opl3w/fmopl3lib/opl3.cpp",
            "native/opl3w/fmopl3lib/opl3class.cpp",
            "native/opl3w/synthlib_opl3w/opl3midi.cpp",
        ])
        .compile("kog_opl3w");

    // Emit this archive after kog_opl3w so static linkers see the dependency
    // after the C++ objects that reference it.
    cc::Build::new()
        .std("c11")
        .include("native/opl3w")
        .file("native/opl3w/resampler.c")
        .compile("kog_opl3w_resampler");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("native/libvgm-kog/kog_libvgm.cpp")
        .include("native/libvgm-kog")
        .include("native/libvgm")
        .warnings(false)
        .compile("kog_libvgm");
    link_libvgm(&libvgm_output);

    println!("cargo:rerun-if-changed=native/opl3w");

    let qt_builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.kog.player").qml_files([
        "qml/CogButton.qml",
        "qml/AudioVisualization.qml",
        "qml/ClassicPlayer.qml",
        "qml/SkinBrowser.qml",
        "qml/SkinSprite.qml",
        "qml/Visualizer.qml",
        "qml/Equalizer.qml",
        "qml/InfoInspector.qml",
        "qml/KineticWheelHandler.qml",
        "qml/Lyrics.qml",
        "qml/Main.qml",
        "qml/MiniPlayer.qml",
        "qml/NowPlayingNotification.qml",
        "qml/PlaylistHeader.qml",
        "qml/PlaylistRow.qml",
        "qml/Preferences.qml",
        "qml/TagEditor.qml",
        "qml/TreeSearchLayout.qml",
    ]))
    // Keep the bridge include root limited to Kog's hand-written integration
    // header instead of recursively tracking the whole repository.
    .crate_include_root(Some("native".to_owned()))
    .qrc_resources([
        "qml/NotificationLayerShell.qml",
        "qml/icons/application-menu.svg",
        "qml/icons/application-menu-light.svg",
        "qml/icons/audio-volume-high.svg",
        "qml/icons/audio-volume-high-light.svg",
        "qml/icons/audio-x-generic.svg",
        "qml/icons/audio-x-generic-light.svg",
        "qml/icons/dialog-information.svg",
        "qml/icons/dialog-information-light.svg",
        "qml/icons/edit-find.svg",
        "qml/icons/edit-find-light.svg",
        "qml/icons/edit-clear-list.svg",
        "qml/icons/edit-clear-list-light.svg",
        "qml/icons/folder-open.svg",
        "qml/icons/folder-open-light.svg",
        "qml/icons/folder.svg",
        "qml/icons/folder-light.svg",
        "qml/icons/go-up.svg",
        "qml/icons/go-up-light.svg",
        "qml/icons/kog.svg",
        "qml/icons/kog-symbolic.svg",
        "qml/icons/media-playback-pause.svg",
        "qml/icons/media-playback-pause-light.svg",
        "qml/icons/media-playback-start.svg",
        "qml/icons/media-playback-start-light.svg",
        "qml/icons/media-playback-stop.svg",
        "qml/icons/media-playback-stop-light.svg",
        "qml/icons/media-playlist-repeat.svg",
        "qml/icons/media-playlist-repeat-light.svg",
        "qml/icons/media-playlist-shuffle.svg",
        "qml/icons/media-playlist-shuffle-light.svg",
        "qml/icons/media-skip-backward.svg",
        "qml/icons/media-skip-backward-light.svg",
        "qml/icons/media-skip-forward.svg",
        "qml/icons/media-skip-forward-light.svg",
        "qml/icons/view-list-tree.svg",
        "qml/icons/view-list-tree-light.svg",
        "qml/icons/view-restore.svg",
        "qml/icons/view-restore-light.svg",
        "qml/icons/window-close.svg",
        "qml/icons/window-close-light.svg",
    ])
    .files([
        "src/app_controller.rs",
        "src/desktop_integration.rs",
        "src/file_tree_model.rs",
        "src/skin_library.rs",
    ])
    .cpp_file("native/kog_desktop_integration.cpp")
    .cpp_file("native/kog_window_state.cpp")
    .cpp_file("native/kog_skin_network.cpp")
    .cpp_file("native/kog_file_tree_search.h")
    .cpp_file("native/kog_file_tree_search.cpp")
    .cpp_file("native/kog_tree_archive.cpp")
    .cpp_file("native/kog_tree_archive_bridge.cpp")
    .qt_module("Concurrent")
    .qt_module("Gui")
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Widgets");
    let session_headers = wayland_session_headers();
    // The archive tree uses the same in-process libarchive as compress-tools.
    let archive = pkg_config::Config::new()
        .probe("libarchive")
        .expect("libarchive development headers are required for archive browsing");
    // SAFETY: only add include paths for the same Qt installation used by
    // cxx-qt, and a define for our own bridge. Do not alter Qt's ABI flags.
    let qt_builder = unsafe {
        qt_builder.cc_builder(|cc| {
            cc.includes(&archive.include_paths);
            if !session_headers.is_empty() {
                cc.includes(&session_headers);
                cc.define("KOG_WAYLAND_SESSION_RESTORE", None);
            }
        })
    };
    qt_builder.build();
}

fn wayland_session_headers() -> Vec<PathBuf> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return Vec::new();
    }
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".to_owned());
    let query = |key: &str| {
        std::process::Command::new(&qmake)
            .args(["-query", key])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let (Some(headers), Some(version)) = (query("QT_INSTALL_HEADERS"), query("QT_VERSION")) else {
        return Vec::new();
    };
    let parts: Vec<u32> = version.split('.').filter_map(|part| part.parse().ok()).collect();
    // QWaylandWindow exists in 6.10, but setSessionRestoreId was added in
    // 6.11. The Flatpak KDE 6.10 SDK must use geometry-only restoration.
    if parts.len() < 2 || (parts[0], parts[1]) < (6, 11) {
        return Vec::new();
    }
    let headers = PathBuf::from(headers);
    let gui = headers.join("QtGui").join(&version);
    let core = headers.join("QtCore").join(&version);
    if !gui.join("QtGui/qpa/qplatformwindow_p.h").is_file() {
        return Vec::new();
    }
    vec![gui.join("QtGui"), gui, core.join("QtCore"), core]
}

fn build_spessasynth_midi() {
    let source = Path::new("native/spessasynth-core/spessasynth_core");
    if !source.join("include/spessasynth/midi/midi.h").is_file() {
        panic!(
            "SpessaSynth Core C submodule is missing; run `git submodule update --init --recursive`"
        );
    }

    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .define("SS_BUILD_SHARED", "OFF")
        .define("SS_BUILD_EXAMPLES", "OFF")
        .define("SS_ENABLE_SF3_VORBIS", "OFF")
        .define("SS_ENABLE_SF3_FLAC", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib");

    let mut bridge = cc::Build::new();
    bridge.std("c11").warnings(true).extra_warnings(true);

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // The submodule generates spessasynth_exports.h only for shared MSVC
        // builds, but its public headers include it under _MSC_VER even in the
        // static build used here. Provide the empty-decoration stand-in that
        // non-MSVC platforms get from the headers themselves.
        let exports = PathBuf::from(std::env::var("OUT_DIR").expect("cargo OUT_DIR is set"))
            .join("spessasynth-exports");
        fs::create_dir_all(&exports).expect("create spessasynth exports directory");
        fs::write(
            exports.join("spessasynth_exports.h"),
            "#define SPESSASYNTH_EXPORTS\n#define SPESSASYNTH_NO_EXPORT\n",
        )
        .expect("write spessasynth_exports.h");
        let exports = exports.display().to_string();
        config.cflag(format!("/I{exports}"));
        bridge.include(exports);
    }

    let output = config.build();

    bridge
        .include(output.join("include"))
        .file("native/spessasynth_midi_bridge.c")
        .compile("kog_spessasynth_midi_bridge");

    println!("cargo:rustc-link-search=native={}/lib", output.display());
    println!("cargo:rustc-link-lib=static=spessasynth");
    pkg_config::Config::new()
        .cargo_metadata(true)
        .probe("zlib")
        .unwrap_or_else(|error| panic!("zlib is required for compressed XMF support: {error}"));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }

    println!("cargo:rerun-if-changed=native/spessasynth-core/spessasynth_core");
    println!("cargo:rerun-if-changed=native/spessasynth_midi_bridge.c");
    println!("cargo:rerun-if-changed=native/spessasynth_midi_bridge.h");
}

fn build_mt32emu() {
    let source = Path::new("native/munt/mt32emu");
    if !source.join("src/c_interface/c_interface.h").is_file() {
        panic!("Munt submodule is missing; run `git submodule update --init --recursive`");
    }

    let output = cmake::Config::new(source)
        .profile("Release")
        .define("libmt32emu_SHARED", "OFF")
        .define("libmt32emu_C_INTERFACE", "ON")
        .define("libmt32emu_CPP_INTERFACE", "ON")
        .define("libmt32emu_WITH_INTERNAL_RESAMPLER", "ON")
        .define("libmt32emu_BUILD_TESTING", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(output.join("include/mt32emu"))
        .file("native/mt32emu_bridge.cpp")
        .warnings(true)
        .extra_warnings(true)
        .compile("kog_mt32emu_bridge");

    // Emit Munt after the bridge archive so one-pass static linkers see the
    // referenced C API before its definitions.
    println!("cargo:rustc-link-search=native={}/lib", output.display());
    println!("cargo:rustc-link-lib=static=mt32emu");

    println!("cargo:rerun-if-changed=native/munt/mt32emu");
    println!("cargo:rerun-if-changed=native/mt32emu_bridge.cpp");
    println!("cargo:rerun-if-changed=native/mt32emu_bridge.h");
}

fn build_adlmidi() {
    let source = Path::new("native/libadlmidi");
    if !source.join("include/adlmidi.h").is_file() {
        panic!("libADLMIDI submodule is missing; run `git submodule update --init --recursive`");
    }

    let output = cmake::Config::new(source)
        .profile("Release")
        .define("libADLMIDI_STATIC", "ON")
        .define("libADLMIDI_SHARED", "OFF")
        .define("WITH_MIDI_SEQUENCER", "ON")
        .define("WITH_EMBEDDED_BANKS", "ON")
        .define("BUILD_NO_GREY_BANKS", "ON")
        .define("WITH_HQ_RESAMPLER", "OFF")
        .define("WITH_XMI_SUPPORT", "ON")
        .define("USE_DOSBOX_EMULATOR", "OFF")
        .define("USE_NUKED_EMULATOR", "ON")
        .define("USE_OPAL_EMULATOR", "OFF")
        .define("USE_JAVA_EMULATOR", "OFF")
        .define("USE_ESFMU_EMULATOR", "OFF")
        .define("USE_MAME_EMULATOR", "OFF")
        .define("USE_YMFM_EMULATOR", "OFF")
        .define("USE_NUKED_OPL2_LLE_EMULATOR", "OFF")
        .define("USE_NUKED_OPL3_LLE_EMULATOR", "OFF")
        .define("USE_HW_SERIAL", "OFF")
        .define("WITH_MIDIPLAY", "OFF")
        .define("WITH_ADLMIDI2", "OFF")
        .define("WITH_OLD_UTILS", "OFF")
        .define("WITH_MUS2MID", "OFF")
        .define("WITH_XMI2MID", "OFF")
        .define("WITH_MIDIDUMP", "OFF")
        .define("WITH_UNIT_TESTS", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();

    println!("cargo:rustc-link-search=native={}/lib", output.display());
    println!("cargo:rustc-link-lib=static=ADLMIDI");
    println!("cargo:rerun-if-changed=native/libadlmidi");
}

fn build_ffmpeg() {
    let libraries = ["libavformat", "libavcodec", "libavutil", "libswresample"];
    let mut includes = Vec::new();
    for library in libraries {
        let metadata = pkg_config::Config::new()
            .cargo_metadata(true)
            .probe(library)
            .unwrap_or_else(|error| {
                panic!(
                    "FFmpeg development library {library} is required; install FFmpeg with pkg-config metadata: {error}"
                )
            });
        includes.extend(metadata.include_paths);
    }
    includes.sort();
    includes.dedup();

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .file("native/ffmpeg_bridge.cpp")
        .warnings(true)
        .extra_warnings(true);
    for include in includes {
        build.include(include);
    }
    build.compile("kog_ffmpeg");

    println!("cargo:rerun-if-changed=native/ffmpeg_bridge.cpp");
    println!("cargo:rerun-if-changed=native/ffmpeg_bridge.h");
}

fn build_game_music_emu() {
    let source = Path::new("native/game-music-emu");
    if !source.join("CMakeLists.txt").is_file() {
        panic!(
            "Game Music Emu submodule is missing; run `git submodule update --init --recursive`"
        );
    }

    let output = cmake::Config::new(source)
        .profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("GME_BUILD_SHARED", "OFF")
        .define("GME_BUILD_STATIC", "ON")
        .define("GME_BUILD_FRAMEWORK", "OFF")
        .define("GME_BUILD_TESTING", "OFF")
        .define("GME_BUILD_EXAMPLES", "OFF")
        .define("GME_ZLIB", "OFF")
        .define("USE_GME_AY", "ON")
        .define("USE_GME_GBS", "ON")
        .define("USE_GME_GYM", "OFF")
        .define("USE_GME_HES", "ON")
        .define("USE_GME_KSS", "ON")
        .define("USE_GME_NSF", "ON")
        .define("USE_GME_NSFE", "ON")
        .define("USE_GME_SAP", "ON")
        .define("USE_GME_SPC", "ON")
        .define("USE_GME_VGM", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();

    println!("cargo:rustc-link-search=native={}/lib", output.display());
    println!("cargo:rustc-link-lib=static=gme");
    println!("cargo:rerun-if-changed=native/game-music-emu");
}

fn build_sfm_helper() {
    let helper = Path::new("native/sfm-helper");
    let source = Path::new("native/cog-gme-sfm");
    if !helper.join("CMakeLists.txt").is_file() || !source.join("gme/Spc_Sfm.cpp").is_file() {
        panic!("Cog GME SFM helper sources are missing from the Kog checkout");
    }

    let source = plain_absolute(source
        .canonicalize()
        .expect("canonicalize the Cog GME SFM source directory"));
    let output_directory =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("sfm-helper");
    let output = cmake::Config::new(helper)
        .out_dir(output_directory)
        .profile("Release")
        .define("COG_GME_SFM_SOURCE", &source)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-sfm-helper.exe"
    } else {
        "kog-sfm-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "Cog GME SFM helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_SFM_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/sfm-helper");
    println!("cargo:rerun-if-changed=native/cog-gme-sfm");
}

fn build_libvgm() -> std::path::PathBuf {
    let source = Path::new("native/libvgm");
    if !source.join("CMakeLists.txt").is_file() {
        panic!("libvgm submodule is missing; run `git submodule update --init --recursive`");
    }

    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .define("BUILD_LIBAUDIO", "OFF")
        .define("BUILD_LIBEMU", "ON")
        .define("BUILD_LIBPLAYER", "ON")
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_PLAYER", "OFF")
        .define("BUILD_VGM2WAV", "OFF")
        .define("LIBRARY_TYPE", "STATIC")
        .define("LINK_STATIC_LIBS", "OFF")
        .define("UTIL_LOADERS", "ON")
        .define("UTIL_THREADING", "OFF")
        .define("UTIL_CHARSET_CONV", "ON")
        .define("USE_SANITIZERS", "OFF")
        .define("SNDEMU__ALL", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib");

    // Keep libvgm's broad chip coverage without linking its GPL-2.0-only
    // YMF278B implementation into Kog's GPL-3.0-or-later executable.
    for device in [
        "SNDEMU_SN76496_ALL",
        "SNDEMU_YM2413_ALL",
        "SNDEMU_YM2612_ALL",
        "SNDEMU_YM2151_ALL",
        "SNDEMU_SEGAPCM_ALL",
        "SNDEMU_RF5C68_ALL",
        "SNDEMU_YM2203_ALL",
        "SNDEMU_YM2608_ALL",
        "SNDEMU_YM2610_ALL",
        "SNDEMU_YM3812_ALL",
        "SNDEMU_YM3526_ALL",
        "SNDEMU_Y8950_ALL",
        "SNDEMU_YMF262_ALL",
        "SNDEMU_YMZ280B_ALL",
        "SNDEMU_YMF271_ALL",
        "SNDEMU_AY8910_ALL",
        "SNDEMU_32X_PWM_ALL",
        "SNDEMU_GAMEBOY_ALL",
        "SNDEMU_NESAPU_ALL",
        "SNDEMU_YMW258_ALL",
        "SNDEMU_UPD7759_ALL",
        "SNDEMU_MSM6258_ALL",
        "SNDEMU_MSM6295_ALL",
        "SNDEMU_K051649_ALL",
        "SNDEMU_K054539_ALL",
        "SNDEMU_C6280_ALL",
        "SNDEMU_C140_ALL",
        "SNDEMU_C219_ALL",
        "SNDEMU_K053260_ALL",
        "SNDEMU_POKEY_ALL",
        "SNDEMU_QSOUND_ALL",
        "SNDEMU_SCSP_ALL",
        "SNDEMU_WSWAN_ALL",
        "SNDEMU_VBOY_VSU_ALL",
        "SNDEMU_SAA1099_ALL",
        "SNDEMU_ES5503_ALL",
        "SNDEMU_ES5506_ALL",
        "SNDEMU_X1_010_ALL",
        "SNDEMU_C352_ALL",
        "SNDEMU_GA20_ALL",
        "SNDEMU_MIKEY_ALL",
        "SNDEMU_K007232_ALL",
        "SNDEMU_K005289_ALL",
        "SNDEMU_MSM5205_ALL",
        "SNDEMU_MSM5232_ALL",
        "SNDEMU_BSMT2000_ALL",
        "SNDEMU_ICS2115_ALL",
    ] {
        config.define(device, "ON");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // libvgm's bundled iconv import library is 32-bit. Use the native
        // Windows charset converter for both 32-bit and 64-bit builds.
        config
            .define("UTIL_CHARCNV_ICONV", "OFF")
            .define("UTIL_CHARCNV_WINAPI", "ON");
    }

    let output = config.build();

    println!("cargo:rerun-if-changed=native/libvgm");
    println!("cargo:rerun-if-changed=native/libvgm-kog");
    output
}

fn link_libvgm(output: &Path) {
    println!("cargo:rustc-link-search=native={}/lib", output.display());
    let target = std::env::var("TARGET").unwrap_or_default();
    let msvc_suffix = if target.contains("msvc") {
        match std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref() {
            Ok("64") => "_Win64",
            Ok("32") => "_Win32",
            value => panic!("unsupported MSVC pointer width for libvgm: {value:?}"),
        }
    } else {
        ""
    };
    for library in ["vgm-player", "vgm-emu", "vgm-utils"] {
        println!("cargo:rustc-link-lib=static={library}{msvc_suffix}");
    }

    if !target.contains("windows") {
        // zlib is already discovered through pkg-config for SpessaSynth and
        // the PSF decoders. Its MSVC archive is named z.lib, not zlib.lib.
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=m");
    }
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=iconv");
    }
}

fn build_openmpt() {
    let source = Path::new("native/openmpt");
    if !source.join("libopenmpt/libopenmpt.h").is_file() {
        panic!("OpenMPT submodule is missing; run `git submodule update --init --recursive`");
    }

    let mut sources = Vec::new();
    for directory in [
        "common",
        "soundlib",
        "soundlib/plugins",
        "soundlib/plugins/dmo",
        "sounddsp",
        "libopenmpt",
    ] {
        sources.extend(cpp_files(&source.join(directory)));
    }
    sources.sort();

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(source)
        .include(source.join("src"))
        .include(source.join("common"))
        .include(source.join("include"))
        .define("LIBOPENMPT_BUILD", None)
        .define("MPT_WITH_MINIZ", None)
        .define("MPT_WITH_MINIMP3", None)
        .define("MPT_WITH_STBVORBIS", None)
        .define("mp3dec_init", "kog_openmpt_mp3dec_init")
        .define("mp3dec_decode_frame", "kog_openmpt_mp3dec_decode_frame")
        .files(sources)
        .warnings(false)
        .compile("kog_openmpt");

    for (name, file, include) in [
        (
            "kog_openmpt_miniz",
            "include/miniz/miniz.c",
            "include/miniz",
        ),
        (
            "kog_openmpt_minimp3",
            "include/minimp3/minimp3.c",
            "include/minimp3",
        ),
        (
            "kog_openmpt_stb_vorbis",
            "include/stb_vorbis/stb_vorbis.c",
            "include/stb_vorbis",
        ),
    ] {
        cc::Build::new()
            .std("c11")
            .include(source.join(include))
            .file(source.join(file))
            .define("mp3dec_init", "kog_openmpt_mp3dec_init")
            .define("mp3dec_decode_frame", "kog_openmpt_mp3dec_decode_frame")
            .warnings(false)
            .compile(name);
    }

    println!("cargo:rerun-if-changed=native/openmpt");
}

fn cpp_files(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("reading {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "cpp"))
        .collect()
}

fn build_hivelytracker() {
    let source = Path::new("native/hivelytracker/Replayer_Windows");
    if !source.join("hvl_replay.h").is_file() {
        panic!("HivelyTracker submodule is missing; run `git submodule update --init --recursive`");
    }

    cc::Build::new()
        .std("c11")
        .include(source)
        .include("native")
        .flag_if_supported("-fcommon")
        .files([
            source.join("hvl_replay.c"),
            source.join("hvl_tables.c"),
            PathBuf::from("native/hively_bridge.c"),
        ])
        .warnings(false)
        .compile("kog_hivelytracker");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=native/hivelytracker/Replayer_Windows");
    println!("cargo:rerun-if-changed=native/hively_bridge.c");
    println!("cargo:rerun-if-changed=native/hively_bridge.h");
}

fn build_vgmstream() {
    let source = Path::new("native/vgmstream");
    if !source.join("CMakeLists.txt").is_file() {
        panic!("vgmstream submodule is missing; run `git submodule update --init --recursive`");
    }

    let target = std::env::var("TARGET").unwrap_or_default();
    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .build_target("libvgmstream")
        .define("BUILD_CLI", "OFF")
        .define("BUILD_V123", "OFF")
        .define("BUILD_AUDACIOUS", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("USE_MPEG", "OFF")
        .define("USE_VORBIS", "OFF")
        .define("USE_FFMPEG", "OFF")
        .define("USE_G7221", "ON")
        .define("USE_G719", "OFF")
        .define("USE_ATRAC9", "OFF")
        .define("USE_CELT", "OFF")
        .define("USE_SPEEX", "OFF");
    if target.contains("windows") {
        config
            .define("BUILD_WINAMP", "OFF")
            .define("BUILD_XMPLAY", "OFF")
            .define("BUILD_FB2K", "OFF");
        if target.contains("msvc") {
            config.cflag("/DVGM_STDIO_UNICODE");
        } else {
            config.cflag("-DVGM_STDIO_UNICODE");
        }
    }
    let output = config.build();

    cc::Build::new()
        .std("c11")
        .include(source.join("src"))
        .file("native/vgmstream_bridge.c")
        .warnings(false)
        .compile("kog_vgmstream_bridge");

    println!(
        "cargo:rustc-link-search=native={}/build/src",
        output.display()
    );
    println!(
        "cargo:rustc-link-search=native={}/build/src/Release",
        output.display()
    );
    if target.contains("msvc") {
        println!("cargo:rustc-link-lib=static=libvgmstream");
    } else {
        println!("cargo:rustc-link-lib=static=vgmstream");
    }
    if !target.contains("windows") {
        println!("cargo:rustc-link-lib=m");
    }

    println!("cargo:rerun-if-changed=native/vgmstream");
    println!("cargo:rerun-if-changed=native/vgmstream_bridge.c");
    println!("cargo:rerun-if-changed=native/vgmstream_bridge.h");
}

fn build_adplug() {
    let adplug = Path::new("native/adplug");
    let binio = Path::new("native/libbinio");
    if !adplug.join("src/adplug.h").is_file() || !binio.join("src/binio.h.in").is_file() {
        panic!("AdPlug submodules are missing; run `git submodule update --init --recursive`");
    }

    let generated = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("kog-libbinio");
    fs::create_dir_all(&generated).expect("create libbinio generated-header directory");
    let binio_header = fs::read_to_string(binio.join("src/binio.h.in"))
        .expect("read libbinio header template")
        .replace("@ENABLE_STRING@", "1")
        .replace("@ENABLE_IOSTREAM@", "1")
        .replace("@ISO_STDLIB@", "1")
        .replace("@WITH_MATH@", "1")
        .replace("@TYPE_INT@", "long long")
        .replace("@TYPE_FLOAT@", "long double");
    fs::write(generated.join("binio.h"), binio_header).expect("write configured libbinio header");
    let adplug_version = fs::read_to_string(adplug.join("src/version.h.in"))
        .expect("read AdPlug version-header template")
        .replace("@VERSION@", "2.3.4-beta");
    fs::write(generated.join("version.h"), adplug_version)
        .expect("write configured AdPlug version header");

    let mut adplug_cpp = cpp_files(&adplug.join("src"));
    adplug_cpp.push(PathBuf::from("native/adplug_bridge.cpp"));
    adplug_cpp.sort();

    let target = std::env::var("TARGET").unwrap_or_default();
    let mut adplug_build = cc::Build::new();
    adplug_build
        .cpp(true)
        .std("c++17")
        .include(adplug.join("src"))
        .include(binio.join("src"))
        .include(&generated)
        .files(adplug_cpp)
        .warnings(false);
    if target.contains("windows") {
        adplug_build
            .define("WIN32", None)
            .define("stricmp", "_stricmp");
    } else {
        adplug_build
            .define("stricmp", "strcasecmp")
            .flag("-include")
            .flag("strings.h");
    }
    define_adplug_opl_symbols(&mut adplug_build);
    adplug_build.compile("kog_adplug");

    let mut adplug_c = cc::Build::new();
    adplug_c
        .std("c11")
        .include(adplug.join("src"))
        .files([
            adplug.join("src/adlibemu.c"),
            adplug.join("src/debug.c"),
            adplug.join("src/fmopl.c"),
            adplug.join("src/nukedopl.c"),
        ])
        .warnings(false);
    if target.contains("windows") {
        adplug_c.define("WIN32", None);
    }
    define_adplug_opl_symbols(&mut adplug_c);
    adplug_c.compile("kog_adplug_c");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(binio.join("src"))
        .include(&generated)
        .files([
            binio.join("src/binio.cpp"),
            binio.join("src/binfile.cpp"),
            binio.join("src/binwrap.cpp"),
            binio.join("src/binstr.cpp"),
        ])
        .warnings(false)
        .compile("kog_libbinio");

    println!("cargo:rerun-if-changed=native/adplug");
    println!("cargo:rerun-if-changed=native/libbinio");
    println!("cargo:rerun-if-changed=native/adplug_bridge.cpp");
    println!("cargo:rerun-if-changed=native/adplug_bridge.h");
}

fn define_adplug_opl_symbols(build: &mut cc::Build) {
    for (symbol, namespaced) in [
        ("OPL3_Generate", "kog_adplug_OPL3_Generate"),
        (
            "OPL3_GenerateResampled",
            "kog_adplug_OPL3_GenerateResampled",
        ),
        ("OPL3_Reset", "kog_adplug_OPL3_Reset"),
        ("OPL3_WriteReg", "kog_adplug_OPL3_WriteReg"),
        ("OPL3_WriteRegBuffered", "kog_adplug_OPL3_WriteRegBuffered"),
        ("OPL3_GenerateStream", "kog_adplug_OPL3_GenerateStream"),
    ] {
        build.define(symbol, namespaced);
    }
}

fn build_libsidplayfp() {
    let source = Path::new("native/libsidplayfp");
    if !source.join("src/sidplayfp/sidplayfp.h").is_file() {
        panic!("libsidplayfp submodule is missing; run `git submodule update --init --recursive`");
    }

    let generated = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("kog-libsidplayfp");
    let generated_api = generated.join("sidplayfp");
    let generated_residfp = generated.join("residfp");
    fs::create_dir_all(&generated_api).expect("create libsidplayfp generated API directory");
    fs::create_dir_all(&generated_residfp).expect("create reSIDfp generated-header directory");

    let sid_version = fs::read_to_string(source.join("src/sidplayfp/sidversion.h.in"))
        .expect("read libsidplayfp version-header template")
        .replace("@LIB_MAJOR@", "2")
        .replace("@LIB_MINOR@", "4")
        // The pinned development suffix is `0a`; this numeric macro is not
        // consumed by the library, while PACKAGE_VERSION carries the exact
        // printable version below.
        .replace("@LIB_LEVEL@", "0");
    fs::write(generated_api.join("sidversion.h"), sid_version)
        .expect("write configured libsidplayfp version header");

    let have_builtin_expect = if std::env::var("TARGET").unwrap_or_default().contains("msvc") {
        "0"
    } else {
        "1"
    };
    let residfp_defs =
        fs::read_to_string(source.join("src/builders/residfp-builder/residfp/siddefs-fp.h.in"))
            .expect("read reSIDfp configuration-header template")
            .replace("@RESID_BRANCH_HINTS@", "1")
            .replace("@HAVE_BUILTIN_EXPECT@", have_builtin_expect)
            .replace("@PACKAGE_VERSION@", "2.4.0a")
            .replace("@RESID_INLINING@", "1")
            .replace("@RESID_INLINE@", "inline");
    fs::write(generated_residfp.join("siddefs-fp.h"), residfp_defs)
        .expect("write configured reSIDfp header");

    let main_sources = [
        "src/EventScheduler.cpp",
        "src/player.cpp",
        "src/psiddrv.cpp",
        "src/mixer.cpp",
        "src/reloc65.cpp",
        "src/sidemu.cpp",
        "src/c64/c64.cpp",
        "src/c64/mmu.cpp",
        "src/c64/VIC_II/mos656x.cpp",
        "src/c64/CPU/mos6510.cpp",
        "src/c64/CPU/mos6510debug.cpp",
        "src/c64/CIA/interrupt.cpp",
        "src/c64/CIA/mos652x.cpp",
        "src/c64/CIA/SerialPort.cpp",
        "src/c64/CIA/timer.cpp",
        "src/c64/CIA/tod.cpp",
        "src/sidplayfp/sidplayfp.cpp",
        "src/sidplayfp/sidbuilder.cpp",
        "src/sidplayfp/SidConfig.cpp",
        "src/sidplayfp/SidInfo.cpp",
        "src/sidplayfp/SidTune.cpp",
        "src/sidplayfp/SidTuneInfo.cpp",
        "src/sidtune/MUS.cpp",
        "src/sidtune/p00.cpp",
        "src/sidtune/prg.cpp",
        "src/sidtune/PSID.cpp",
        "src/sidtune/SidTuneBase.cpp",
        "src/sidtune/SidTuneTools.cpp",
        "src/utils/iniParser.cpp",
        "src/utils/md5Factory.cpp",
        "src/utils/SidDatabase.cpp",
        "src/utils/MD5/MD5.cpp",
        "src/builders/residfp-builder/residfp-builder.cpp",
        "src/builders/residfp-builder/residfp-emu.cpp",
        "src/builders/residfp-builder/residfp/Dac.cpp",
        "src/builders/residfp-builder/residfp/EnvelopeGenerator.cpp",
        "src/builders/residfp-builder/residfp/ExternalFilter.cpp",
        "src/builders/residfp-builder/residfp/Filter.cpp",
        "src/builders/residfp-builder/residfp/Filter6581.cpp",
        "src/builders/residfp-builder/residfp/Filter8580.cpp",
        "src/builders/residfp-builder/residfp/FilterModelConfig6581.cpp",
        "src/builders/residfp-builder/residfp/FilterModelConfig8580.cpp",
        "src/builders/residfp-builder/residfp/Integrator6581.cpp",
        "src/builders/residfp-builder/residfp/Integrator8580.cpp",
        "src/builders/residfp-builder/residfp/OpAmp.cpp",
        "src/builders/residfp-builder/residfp/SID.cpp",
        "src/builders/residfp-builder/residfp/Spline.cpp",
        "src/builders/residfp-builder/residfp/WaveformCalculator.cpp",
        "src/builders/residfp-builder/residfp/WaveformGenerator.cpp",
        "src/builders/residfp-builder/residfp/resample/SincResampler.cpp",
        "src/builders/residfp-builder/residfp/version.cc",
    ];

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(&generated)
        .include(&generated_residfp)
        .include(source.join("src"))
        .include(source.join("src/sidtune"))
        .include("native/libsidplayfp-generated/sidtune")
        .include(source.join("src/builders/residfp-builder"))
        .include(source.join("src/builders/residfp-builder/residfp"))
        .define("HAVE_CXX11", None)
        .define("HAVE_CXX14", None)
        .define("PACKAGE_NAME", "\"libsidplayfp\"")
        .define("PACKAGE_VERSION", "\"2.4.0a\"")
        .define("PACKAGE_URL", "\"https://github.com/kode54/libsidplayfp\"")
        .define("VERSION", "\"2.4.0a\"")
        .files(main_sources.iter().map(|path| source.join(path)))
        .file("native/sid_bridge.cpp")
        .warnings(false)
        .compile("kog_libsidplayfp");

    println!("cargo:rerun-if-changed=native/libsidplayfp");
    println!("cargo:rerun-if-changed=native/libsidplayfp-generated");
    println!("cargo:rerun-if-changed=native/sid_bridge.cpp");
    println!("cargo:rerun-if-changed=native/sid_bridge.h");
}

fn build_mgba() -> PathBuf {
    let source = Path::new("native/mgba");
    if !source.join("CMakeLists.txt").is_file() {
        panic!("mGBA submodule is missing; run `git submodule update --init --recursive`");
    }

    let output_directory =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("mgba");
    let mut config = cmake::Config::new(source);
    config
        .out_dir(output_directory)
        .profile("Release")
        .define("BUILD_STATIC", "ON")
        .define("BUILD_SHARED", "OFF")
        .define("DISABLE_FRONTENDS", "ON")
        .define("DISABLE_DEPS", "ON")
        // Kog links only the core; upstream's Windows path demands an epoxy
        // acknowledgement unless the library-only build is selected.
        .define("LIBMGBA_ONLY", "ON")
        .define("M_CORE_GBA", "ON")
        .define("M_CORE_GB", "OFF")
        .define("ENABLE_DEBUGGERS", "OFF")
        .define("ENABLE_GDB_STUB", "OFF")
        .define("ENABLE_SCRIPTING", "OFF")
        .define("BUILD_QT", "OFF")
        .define("BUILD_SDL", "OFF")
        .define("BUILD_GL", "OFF")
        .define("BUILD_GLES2", "OFF")
        .define("BUILD_GLES3", "OFF")
        .define("BUILD_TEST", "OFF")
        .define("BUILD_SUITE", "OFF")
        .define("BUILD_CINEMA", "OFF")
        .define("BUILD_HEADLESS", "OFF")
        .define("BUILD_EXAMPLE", "OFF")
        .define("BUILD_PYTHON", "OFF")
        .define("BUILD_LIBRETRO", "OFF")
        .define("BUILD_LTO", "OFF")
        .define("USE_FFMPEG", "OFF")
        .define("USE_ZLIB", "OFF")
        .define("USE_MINIZIP", "OFF")
        .define("USE_PNG", "OFF")
        .define("USE_LIBZIP", "OFF")
        .define("USE_SQLITE3", "OFF")
        .define("USE_ELF", "OFF")
        .define("USE_LUA", "OFF")
        .define("USE_JSON_C", "OFF")
        .define("USE_FREETYPE", "OFF")
        .define("USE_LZMA", "OFF")
        .define("USE_DISCORD_RPC", "OFF")
        // mGBA replaces CMAKE_INSTALL_LIBDIR with "." on native Windows.
        // Set its project-specific destination so Cargo can find mgba.lib in
        // the same installed lib directory used on Unix.
        .define("LIBDIR", "lib")
        .define("CMAKE_INSTALL_LIBDIR", "lib");
    // mGBA's utility CRC function otherwise collides with zlib's public
    // crc32 symbol when both static archives are linked into Kog.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        config.cflag("/Dcrc32=kog_mgba_crc32");
    } else {
        config.cflag("-Dcrc32=kog_mgba_crc32");
    }
    let output = config.build();

    println!("cargo:rerun-if-changed=native/mgba");
    output
}

fn build_ncsf(mgba_output: &Path) {
    let player = Path::new("native/sseqplayer");
    let psflib = Path::new("native/psflib");
    let qsf_core = Path::new("native/highly-quixotic/Core");
    let sdsf_core = Path::new("native/highly-theoretical/Core");
    let usf_core = Path::new("native/lazyusf2");
    if !player.join("Player.h").is_file()
        || !psflib.join("psflib.h").is_file()
        || !qsf_core.join("qsound.h").is_file()
        || !sdsf_core.join("sega.h").is_file()
        || !usf_core.join("usf/usf.h").is_file()
    {
        panic!(
            "SSEQPlayer, psflib, Highly Quixotic, Highly Theoretical, or LazyUSF2 submodules are missing; run `git submodule update --init --recursive`"
        );
    }

    let zlib = pkg_config::Config::new()
        .cargo_metadata(true)
        .probe("zlib")
        .unwrap_or_else(|error| panic!("zlib is required for NCSF/psflib support: {error}"));

    let mut player_sources = cpp_files(player);
    player_sources.push(PathBuf::from("native/ncsf_bridge.cpp"));
    player_sources.push(PathBuf::from("native/qsf_bridge.cpp"));
    player_sources.push(PathBuf::from("native/sdsf_bridge.cpp"));
    player_sources.push(PathBuf::from("native/usf_bridge.cpp"));
    player_sources.sort();
    let mut player_build = cc::Build::new();
    player_build
        .cpp(true)
        .std("c++17")
        .include(player)
        .include(psflib)
        .include(qsf_core)
        .include(sdsf_core)
        .include(usf_core)
        .include(usf_core.join("usf"))
        .include(mgba_output.join("include"))
        // This structural feature is present in mGBA's target compile
        // definitions but omitted from its generated flags.h at this pin.
        .define("ENABLE_DIRECTORIES", None)
        .file("native/gsf_bridge.cpp")
        .files(player_sources)
        .warnings(false);
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        // SSEQPlayer's pre-C++11 libstdc++ shims conflict with modern GNU
        // libstdc++. libc++ and MSVC select their own standard implementation.
        player_build.define("_LIBCPP_VERSION", "1");
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // mGBA's headers pull in windows.h, whose min/max macros collide
        // with the std:: forms used across these bridges. Every TU here
        // also compiles without the macro on Linux, so this is safe.
        player_build.define("NOMINMAX", None);
    }
    player_build.compile("kog_sseqplayer");

    // Highly Quixotic is Cog's QSF engine. Generate two narrowly patched C
    // translation units: upstream's non-static C99 inline helpers otherwise
    // fail to link, and its original ROM access assumes trusted game data.
    // Keeping these changes in OUT_DIR leaves the pinned dependency pristine.
    let qsf_generated = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("highly-quixotic");
    fs::create_dir_all(&qsf_generated).expect("create Highly Quixotic generated directory");

    let qsound_source = fs::read_to_string(qsf_core.join("qsound.c"))
        .expect("read Highly Quixotic qsound.c")
        .replace("\r\n", "\n");
    let original_banked_maps = r#"static void recompute_banked_rom_areas(struct QSOUND_STATE *state) {
  set_memory_map_rom_area(
    state->map_op + 1,
    state->z80_rom      + state->bank_ofs,
    state->z80_rom_size - state->bank_ofs
  );
  set_memory_map_rom_area(
    state->map_read + 1,
    state->z80_rom      + state->bank_ofs,
    state->z80_rom_size - state->bank_ofs
  );
}"#;
    let bounded_banked_maps = r#"static void recompute_banked_rom_areas(struct QSOUND_STATE *state) {
  uint8 *bank_rom = safe_rom_area;
  sint32 bank_rom_size = 0;
  if(state->z80_rom != NULL && state->bank_ofs < state->z80_rom_size) {
    bank_rom = state->z80_rom + state->bank_ofs;
    bank_rom_size = (sint32)(state->z80_rom_size - state->bank_ofs);
  }
  set_memory_map_rom_area(state->map_op + 1, bank_rom, bank_rom_size);
  set_memory_map_rom_area(state->map_read + 1, bank_rom, bank_rom_size);
}"#;
    assert!(
        qsound_source.contains(original_banked_maps),
        "Highly Quixotic qsound.c changed; re-audit Kog's banked-ROM safety patch"
    );
    fs::write(
        qsf_generated.join("qsound.c"),
        qsound_source.replace(original_banked_maps, bounded_banked_maps),
    )
    .expect("write bounded Highly Quixotic qsound.c");

    let qsound_ctr_source = fs::read_to_string(qsf_core.join("qsound_ctr.c"))
        .expect("read Highly Quixotic qsound_ctr.c")
        .replace("\r\n", "\n");
    assert!(
        qsound_ctr_source.contains("#define INLINE __inline")
            && qsound_ctr_source.contains("#define INLINE inline"),
        "Highly Quixotic qsound_ctr.c changed; re-audit Kog's inline patch"
    );
    let original_sample_read =
        "\trom_addr = (bank << 16) | (address << 0);\n\t\n\tsample_data = chip->romData[rom_addr];";
    let bounded_sample_read = "\trom_addr = (bank << 16) | (address << 0);\n\tif (rom_addr >= chip->romSize)\n\t\treturn 0;\n\t\n\tsample_data = chip->romData[rom_addr];";
    assert!(
        qsound_ctr_source.contains(original_sample_read),
        "Highly Quixotic qsound_ctr.c changed; re-audit Kog's sample-ROM safety patch"
    );
    let original_rom_allocation = "\tchip->romData = (UINT8*)realloc(chip->romData, memsize);\n\tchip->romSize = memsize;\n\tchip->romMask = pow2_mask(memsize);\n\tmemset(chip->romData, 0xFF, memsize);";
    let checked_rom_allocation = "\tUINT8* newRomData = (UINT8*)realloc(chip->romData, memsize);\n\tif (!newRomData)\n\t{\n\t\tfree(chip->romData);\n\t\tchip->romData = NULL;\n\t\tchip->romSize = 0;\n\t\tchip->romMask = 0;\n\t\treturn;\n\t}\n\tchip->romData = newRomData;\n\tchip->romSize = memsize;\n\tchip->romMask = pow2_mask(memsize);\n\tmemset(chip->romData, 0xFF, memsize);";
    assert!(
        qsound_ctr_source.contains(original_rom_allocation),
        "Highly Quixotic qsound_ctr.c changed; re-audit Kog's sample-ROM allocation patch"
    );
    let original_rom_write_guard = "\tif (offset > chip->romSize)\n\t\treturn;";
    let checked_rom_write_guard = "\tif (!chip->romData || offset >= chip->romSize)\n\t\treturn;";
    assert!(
        qsound_ctr_source.contains(original_rom_write_guard),
        "Highly Quixotic qsound_ctr.c changed; re-audit Kog's sample-ROM write patch"
    );
    let qsound_ctr_source = qsound_ctr_source
        .replace("#define INLINE __inline", "#define INLINE static __inline")
        .replace("#define INLINE inline", "#define INLINE static inline")
        .replace(original_sample_read, bounded_sample_read)
        .replace(original_rom_allocation, checked_rom_allocation)
        .replace(original_rom_write_guard, checked_rom_write_guard);
    let qsound_ctr_source = format!(
        "{qsound_ctr_source}\n\nint kog_qsoundc_has_rom(void* info, UINT32 size)\n{{\n\tstruct qsound_chip* chip = (struct qsound_chip*)info;\n\treturn chip && chip->romData && chip->romSize == size;\n}}\n\nvoid kog_qsoundc_cleanup(void* info)\n{{\n\tstruct qsound_chip* chip = (struct qsound_chip*)info;\n\tif (!chip)\n\t\treturn;\n\tfree(chip->romData);\n\tchip->romData = NULL;\n\tchip->romSize = 0;\n\tchip->romMask = 0;\n}}\n"
    );
    fs::write(qsf_generated.join("qsound_ctr.c"), qsound_ctr_source)
        .expect("write bounded Highly Quixotic qsound_ctr.c");

    let mut qsf_build = cc::Build::new();
    qsf_build
        .std("c11")
        .include(qsf_core)
        .define("EMU_COMPILE", None)
        .define("HAVE_STDINT_H", None)
        .files([
            qsf_generated.join("qsound.c"),
            qsf_generated.join("qsound_ctr.c"),
            qsf_core.join("kabuki.c"),
            qsf_core.join("z80.c"),
        ])
        .warnings(false);
    match std::env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() {
        Ok("big") => qsf_build.define("EMU_BIG_ENDIAN", None),
        _ => qsf_build.define("EMU_LITTLE_ENDIAN", None),
    };
    qsf_build.compile("kog_highly_quixotic");

    // Highly Theoretical is Cog's underlying SSF/DSF engine. Build its
    // GPLv2-or-later C68k path rather than the separately licensed optional
    // Musashi or Starscream cores. Two generated source patches preserve the
    // pinned submodule while fixing a pointer conversion and modern 64-bit
    // portability warnings already addressed by Cog's local copy.
    let sdsf_generated = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("highly-theoretical");
    fs::create_dir_all(&sdsf_generated).expect("create Highly Theoretical generated directory");

    let satsound_source = fs::read_to_string(sdsf_core.join("satsound.c"))
        .expect("read Highly Theoretical satsound.c")
        .replace("\r\n", "\n");
    let original_fetch = "C68k_Set_Fetch(SCPUSTATE, 0x00000, 0x7FFFF, RAMBYTEPTR);";
    assert_eq!(
        satsound_source.matches(original_fetch).count(),
        2,
        "Highly Theoretical satsound.c changed; re-audit Kog's C68k pointer patch"
    );
    fs::write(
        sdsf_generated.join("satsound.c"),
        satsound_source.replace(
            original_fetch,
            "C68k_Set_Fetch(SCPUSTATE, 0x00000, 0x7FFFF, (pointer)RAMBYTEPTR);",
        ),
    )
    .expect("write portable Highly Theoretical satsound.c");

    let yam_source = fs::read_to_string(sdsf_core.join("yam.c"))
        .expect("read Highly Theoretical yam.c")
        .replace("\r\n", "\n");
    let original_calling_convention =
        "#ifndef _WIN32\n#define __cdecl\n#define __fastcall __attribute__((regparm(3)))\n#endif";
    let portable_calling_convention = "#ifndef _WIN32\n#define __cdecl\n#ifdef __aarch64__\n#define __fastcall\n#else\n#define __fastcall __attribute__((regparm(3)))\n#endif\n#endif";
    assert!(
        yam_source.contains(original_calling_convention),
        "Highly Theoretical yam.c changed; re-audit Kog's calling-convention patch"
    );
    fs::write(
        sdsf_generated.join("yam.c"),
        yam_source.replace(original_calling_convention, portable_calling_convention),
    )
    .expect("write portable Highly Theoretical yam.c");

    let mut sdsf_build = cc::Build::new();
    sdsf_build
        .std("c11")
        .include(sdsf_core)
        .include(sdsf_core.join("c68k"))
        .define("EMU_COMPILE", None)
        .define("HAVE_STDINT_H", None)
        .define("C68K_NO_JUMP_TABLE", None)
        .files([
            sdsf_core.join("sega.c"),
            sdsf_core.join("dcsound.c"),
            sdsf_generated.join("satsound.c"),
            sdsf_generated.join("yam.c"),
            sdsf_core.join("arm.c"),
            sdsf_core.join("c68k/c68k.c"),
            sdsf_core.join("c68k/c68kexec.c"),
        ])
        .warnings(false);
    match std::env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() {
        Ok("big") => sdsf_build.define("EMU_BIG_ENDIAN", None),
        _ => sdsf_build.define("EMU_LITTLE_ENDIAN", None),
    };
    sdsf_build.compile("kog_highly_theoretical");

    // LazyUSF2 is Cog's underlying USF engine. Its upstream CMake file is
    // hard-coded to x86-64, so select the maintained dynarec only where it is
    // supported and retain the cached interpreter on every other target.
    let mut usf_sources = vec![
        "ai/ai_controller.c",
        "api/callbacks.c",
        "debugger/dbg_decoder.c",
        "main/main.c",
        "main/rom.c",
        "main/savestates.c",
        "main/util.c",
        "memory/memory.c",
        "pi/cart_rom.c",
        "pi/pi_controller.c",
        "r4300/cached_interp.c",
        "r4300/cp0.c",
        "r4300/cp1.c",
        "r4300/exception.c",
        "r4300/interupt.c",
        "r4300/mi_controller.c",
        "r4300/pure_interp.c",
        "r4300/r4300.c",
        "r4300/r4300_core.c",
        "r4300/recomp.c",
        "r4300/reset.c",
        "r4300/tlb.c",
        "rdp/rdp_core.c",
        "ri/rdram.c",
        "ri/rdram_detection_hack.c",
        "ri/ri_controller.c",
        "rsp/rsp_core.c",
        "rsp_hle/alist.c",
        "rsp_hle/alist_audio.c",
        "rsp_hle/alist_naudio.c",
        "rsp_hle/alist_nead.c",
        "rsp_hle/audio.c",
        "rsp_hle/cicx105.c",
        "rsp_hle/hle.c",
        "rsp_hle/hvqm.c",
        "rsp_hle/jpeg.c",
        "rsp_hle/memory.c",
        "rsp_hle/mp3.c",
        "rsp_hle/musyx.c",
        "rsp_hle/plugin.c",
        "rsp_hle/re2.c",
        "rsp_lle/rsp.c",
        "si/cic.c",
        "si/game_controller.c",
        "si/n64_cic_nus_6105.c",
        "si/pif.c",
        "si/si_controller.c",
        "usf/barray.c",
        "usf/resampler.c",
        "usf/usf.c",
        "vi/vi_controller.c",
    ];
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let mut usf_build = cc::Build::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        usf_build.std("c11");
        // cl does not predefine WIN32 (only _WIN32); lazyusf2's m64p API
        // headers select their MSVC declarations with it, and otherwise
        // emit GCC __attribute syntax that MSVC cannot parse.
        usf_build.define("WIN32", None);
        if std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref() == Ok("64") {
            // LazyUSF2 calls __control87_2, which the MSVC CRT does not
            // provide on 64-bit targets. Force-include Kog's compatible
            // _controlfp_s wrapper before LazyUSF2's fpu.h.
            usf_build
                .include("native")
                .flag("/FIlazyusf2_msvc_fenv.h");
        }
    } else {
        // Upstream's CMake build uses compiler extensions and relies on the
        // POSIX strdup declaration exposed by GNU/Clang's gnu11 mode.
        usf_build.std("gnu11");
    }
    usf_build.include(usf_core).warnings(false);
    // lazyusf2's interpreter tables include zlib.h; MSVC has no default
    // system include path for it.
    for include in &zlib.include_paths {
        usf_build.include(include);
    }
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    match target_arch.as_str() {
        // The x86 dynarecs are written in GNU inline assembly, which MSVC
        // cannot assemble; MSVC takes the portable interpreter instead.
        "x86_64" if !msvc => {
            usf_build
                .define("DYNAREC", None)
                .define("ARCH_MIN_SSE2", None);
            usf_sources.extend([
                "r4300/x86_64/assemble.c",
                "r4300/x86_64/gbc.c",
                "r4300/x86_64/gcop0.c",
                "r4300/x86_64/gcop1.c",
                "r4300/x86_64/gcop1_d.c",
                "r4300/x86_64/gcop1_l.c",
                "r4300/x86_64/gcop1_s.c",
                "r4300/x86_64/gcop1_w.c",
                "r4300/x86_64/gr4300.c",
                "r4300/x86_64/gregimm.c",
                "r4300/x86_64/gspecial.c",
                "r4300/x86_64/gtlb.c",
                "r4300/x86_64/regcache.c",
                "r4300/x86_64/rjump.c",
            ]);
        }
        "x86" if !msvc => {
            usf_build
                .define("DYNAREC", None)
                .define("ARCH_MIN_SSE2", None);
            usf_sources.extend([
                "r4300/x86/assemble.c",
                "r4300/x86/gbc.c",
                "r4300/x86/gcop0.c",
                "r4300/x86/gcop1.c",
                "r4300/x86/gcop1_d.c",
                "r4300/x86/gcop1_l.c",
                "r4300/x86/gcop1_s.c",
                "r4300/x86/gcop1_w.c",
                "r4300/x86/gr4300.c",
                "r4300/x86/gregimm.c",
                "r4300/x86/gspecial.c",
                "r4300/x86/gtlb.c",
                "r4300/x86/regcache.c",
                "r4300/x86/rjump.c",
            ]);
        }
        "aarch64" => {
            usf_build.define("ARCH_MIN_ARM_NEON", None);
            usf_sources.push("r4300/empty_dynarec.c");
        }
        _ => usf_sources.push("r4300/empty_dynarec.c"),
    }
    usf_build.files(usf_sources.iter().map(|source| usf_core.join(source)));
    usf_build.compile("kog_lazyusf2");

    // Emit psflib after the C++ archive so one-pass static linkers see the
    // parser dependency after the NCSF, GSF, and QSF bridge references.
    let mut psf_build = cc::Build::new();
    psf_build
        .std("c11")
        .include(psflib)
        .file(psflib.join("psflib.c"))
        .warnings(false);
    for include in &zlib.include_paths {
        psf_build.include(include);
    }
    psf_build.compile("kog_psflib");

    println!(
        "cargo:rustc-link-search=native={}/lib",
        mgba_output.display()
    );
    println!("cargo:rustc-link-lib=static=mgba");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=ws2_32");
        println!("cargo:rustc-link-lib=shlwapi");
    } else {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    println!("cargo:rerun-if-changed=native/sseqplayer");
    println!("cargo:rerun-if-changed=native/psflib");
    println!("cargo:rerun-if-changed=native/highly-quixotic");
    println!("cargo:rerun-if-changed=native/highly-theoretical");
    println!("cargo:rerun-if-changed=native/lazyusf2");
    println!("cargo:rerun-if-changed=native/lazyusf2_msvc_fenv.h");
    println!("cargo:rerun-if-changed=native/ncsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/ncsf_bridge.h");
    println!("cargo:rerun-if-changed=native/gsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/gsf_bridge.h");
    println!("cargo:rerun-if-changed=native/qsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/qsf_bridge.h");
    println!("cargo:rerun-if-changed=native/sdsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/sdsf_bridge.h");
    println!("cargo:rerun-if-changed=native/usf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/usf_bridge.h");
}

fn build_psf_helper() {
    let helper = Path::new("native/psf-helper");
    let libupse = Path::new("native/libupse");
    if !helper.join("CMakeLists.txt").is_file() || !libupse.join("upse.h").is_file() {
        panic!(
            "libupse PSF helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let libupse = plain_absolute(libupse
        .canonicalize()
        .expect("canonicalize the libupse source directory"));
    let output = cmake::Config::new(helper)
        .profile("Release")
        .define("UPSE_SOURCE", &libupse)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-psf-helper.exe"
    } else {
        "kog-psf-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "libupse PSF helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_PSF_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/psf-helper");
    println!("cargo:rerun-if-changed=native/libupse");
}

fn build_psf2_helper() {
    let helper = Path::new("native/psf2-helper");
    let play = Path::new("native/play");
    if !helper.join("CMakeLists.txt").is_file()
        || !play.join("License.txt").is_file()
        || !play
            .join("deps/Framework/build_cmake/Framework/CMakeLists.txt")
            .exists()
        || !play.join("deps/CodeGen/CMakeLists.txt").exists()
        || !play
            .join("deps/Dependencies/zstd/build/cmake/CMakeLists.txt")
            .exists()
    {
        panic!(
            "Play! PSF2 helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let play = plain_absolute(play
        .canonicalize()
        .expect("canonicalize the Play! source directory"));
    let output_directory =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("psf2-helper");
    let mut config = cmake::Config::new(helper);
    config
        .out_dir(output_directory)
        .profile("Release")
        .define("PLAY_SOURCE", &play)
        .define("ENABLE_AMAZON_S3", "OFF")
        // libchdr's bundled zlib CMake renames a tracked source header during
        // out-of-tree configuration. Use the system zlib so Cargo builds stay
        // reproducible and the pinned Play! submodule remains immutable.
        .define("WITH_SYSTEM_ZLIB", "ON");
    let output = config.build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-psf2-helper.exe"
    } else {
        "kog-psf2-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "Play! PSF2 helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_PSF2_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/psf2-helper");
    println!("cargo:rerun-if-changed=native/play");
}

fn build_twosf_helper() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The pinned melonDS core leans on GCC-only constructs (inline asm,
        // variable-length arrays, __builtin_*), so it cannot be compiled by
        // MSVC without patching the submodule. Emit the expected build path so
        // psf.rs compiles; playback reports a clear error unless the user
        // supplies a helper via KOG_2SF_HELPER or a sibling binary.
        let unbuilt = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
            .join("twosf-helper")
            .join("bin")
            .join("kog-2sf-helper.exe");
        println!(
            "cargo:rustc-env=KOG_BUILD_2SF_HELPER={}",
            unbuilt.display()
        );
        println!("cargo:rerun-if-changed=native/twosf-helper");
        println!("cargo:rerun-if-changed=native/melonds");
        return;
    }

    let helper = Path::new("native/twosf-helper");
    let melonds = Path::new("native/melonds");
    let psflib = Path::new("native/psflib");
    if !helper.join("CMakeLists.txt").is_file()
        || !melonds.join("src/NDS.h").is_file()
        || !psflib.join("psflib.h").is_file()
    {
        panic!(
            "melonDS 2SF helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let melonds = plain_absolute(melonds
        .canonicalize()
        .expect("canonicalize the melonDS source directory"));
    let psflib = plain_absolute(psflib
        .canonicalize()
        .expect("canonicalize the psflib source directory"));
    let output_directory = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("twosf-helper");
    let output = cmake::Config::new(helper)
        .out_dir(output_directory)
        .profile("Release")
        .define("MELONDS_SOURCE", &melonds)
        .define("PSFLIB_SOURCE", &psflib)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-2sf-helper.exe"
    } else {
        "kog-2sf-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "melonDS 2SF helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_2SF_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/twosf-helper");
    println!("cargo:rerun-if-changed=native/melonds");
}

fn build_snsf_helper() {
    let helper = Path::new("native/snsf-helper");
    let libsnsf9x = Path::new("native/libsnsf9x");
    let psflib = Path::new("native/psflib");
    if !helper.join("CMakeLists.txt").is_file()
        || !libsnsf9x.join("snsf9x.h").is_file()
        || !psflib.join("psflib.h").is_file()
    {
        panic!(
            "libsnsf9x SNSF helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let libsnsf9x = plain_absolute(libsnsf9x
        .canonicalize()
        .expect("canonicalize the libsnsf9x source directory"));
    let psflib = plain_absolute(psflib
        .canonicalize()
        .expect("canonicalize the psflib source directory"));
    let output_directory =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("snsf-helper");
    let output = cmake::Config::new(helper)
        .out_dir(output_directory)
        .profile("Release")
        .define("LIBSNSF9X_SOURCE", &libsnsf9x)
        .define("PSFLIB_SOURCE", &psflib)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-snsf-helper.exe"
    } else {
        "kog-snsf-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "libsnsf9x SNSF helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_SNSF_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/snsf-helper");
    println!("cargo:rerun-if-changed=native/libsnsf9x");
}

fn build_syntrax_helper() {
    let helper = Path::new("native/syntrax-helper");
    let syntrax = Path::new("native/syntrax-c");
    if !helper.join("CMakeLists.txt").is_file() || !syntrax.join("jaytrax.c").is_file() {
        panic!(
            "syntrax-c JXS helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let syntrax = plain_absolute(syntrax
        .canonicalize()
        .expect("canonicalize the syntrax-c source directory"));
    let output_directory = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("syntrax-helper");
    let output = cmake::Config::new(helper)
        .out_dir(output_directory)
        .profile("Release")
        .define("SYNTRAX_SOURCE", &syntrax)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-syntrax-helper.exe"
    } else {
        "kog-syntrax-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "syntrax-c JXS helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_SYNTRAX_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/syntrax-helper");
    println!("cargo:rerun-if-changed=native/syntrax-c");
}

fn build_sc55_helper() {
    let helper = Path::new("native/sc55-helper");
    let nuked_sc55 = Path::new("native/nuked-sc55");
    if !helper.join("CMakeLists.txt").is_file() || !nuked_sc55.join("src/backend/emu.h").is_file() {
        panic!(
            "Nuked SC-55 helper sources are missing; run `git submodule update --init --recursive`"
        );
    }

    let nuked_sc55 = plain_absolute(nuked_sc55
        .canonicalize()
        .expect("canonicalize the Nuked SC-55 source directory"));
    let output_directory =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("sc55-helper");
    let output = cmake::Config::new(helper)
        .out_dir(output_directory)
        .profile("Release")
        .define("NUKED_SC55_SOURCE", &nuked_sc55)
        .build();
    let executable_name = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "kog-sc55-helper.exe"
    } else {
        "kog-sc55-helper"
    };
    let executable = output.join("bin").join(executable_name);
    if !executable.is_file() {
        panic!(
            "Nuked SC-55 helper build did not install {}",
            executable.display()
        );
    }

    println!(
        "cargo:rustc-env=KOG_BUILD_SC55_HELPER={}",
        executable.display()
    );
    println!("cargo:rerun-if-changed=native/sc55-helper");
    println!("cargo:rerun-if-changed=native/nuked-sc55");
}
