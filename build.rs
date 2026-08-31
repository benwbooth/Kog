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

    let output = config.build();

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
    if !player.join("Player.h").is_file()
        || !psflib.join("psflib.h").is_file()
        || !qsf_core.join("qsound.h").is_file()
    {
        panic!(
            "SSEQPlayer, psflib, or Highly Quixotic submodules are missing; run `git submodule update --init --recursive`"
        );
    }

    let zlib = pkg_config::Config::new()
        .cargo_metadata(true)
        .probe("zlib")
        .unwrap_or_else(|error| panic!("zlib is required for NCSF/psflib support: {error}"));

    let mut player_sources = cpp_files(player);
    player_sources.push(PathBuf::from("native/ncsf_bridge.cpp"));
    player_sources.push(PathBuf::from("native/qsf_bridge.cpp"));
    player_sources.sort();
    let mut player_build = cc::Build::new();
    player_build
        .cpp(true)
        .std("c++17")
        .include(player)
        .include(psflib)
        .include(qsf_core)
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
    println!("cargo:rerun-if-changed=native/ncsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/ncsf_bridge.h");
    println!("cargo:rerun-if-changed=native/gsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/gsf_bridge.h");
    println!("cargo:rerun-if-changed=native/qsf_bridge.cpp");
    println!("cargo:rerun-if-changed=native/qsf_bridge.h");
}
