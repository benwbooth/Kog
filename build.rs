use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    build_game_music_emu();
    let libvgm_output = build_libvgm();
    build_openmpt();
    build_hivelytracker();
    build_vgmstream();

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

    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.kog.player")
            .depend("QtQuick.Dialogs")
            .qml_files([
                "qml/CogButton.qml",
                "qml/InfoInspector.qml",
                "qml/Main.qml",
                "qml/MiniPlayer.qml",
                "qml/PlaylistHeader.qml",
                "qml/PlaylistRow.qml",
                "qml/Preferences.qml",
            ]),
    )
    .files(["src/app_controller.rs"])
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .build();
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

fn build_libvgm() -> std::path::PathBuf {
    let source = Path::new("native/libvgm");
    if !source.join("CMakeLists.txt").is_file() {
        panic!("libvgm submodule is missing; run `git submodule update --init --recursive`");
    }

    let output = cmake::Config::new(source)
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
        .define("SNDEMU__ALL", "ON")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();

    println!("cargo:rerun-if-changed=native/libvgm");
    println!("cargo:rerun-if-changed=native/libvgm-kog");
    output
}

fn link_libvgm(output: &Path) {
    println!("cargo:rustc-link-search=native={}/lib", output.display());
    println!("cargo:rustc-link-lib=static=vgm-player");
    println!("cargo:rustc-link-lib=static=vgm-emu");
    println!("cargo:rustc-link-lib=static=vgm-utils");

    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=zlib");
    } else {
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
