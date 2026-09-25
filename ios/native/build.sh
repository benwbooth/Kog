#!/usr/bin/env bash
set -euo pipefail

# Cross-build the same Rust decoder used by Qt, TUI, Web, and Android for iOS.
# The Swift app links the resulting static archive into the application binary.
platform=${1:-device}
case "$platform" in
  device) target=aarch64-apple-ios; sdk_name=iphoneos; sdk_platform=iPhoneOS; deployment=arm64-apple-ios17.0 ;;
  simulator) target=aarch64-apple-ios-sim; sdk_name=iphonesimulator; sdk_platform=iPhoneSimulator; deployment=arm64-apple-ios17.0-simulator ;;
  *) echo "Usage: $0 [device|simulator]" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "$0")/../.." && pwd)
for tool in xcrun cmake ninja pkg-config curl git make cargo rustup; do
  command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 2; }
done
sdk=$(xcrun --sdk "$sdk_name" --show-sdk-path)
clang=$(xcrun --sdk "$sdk_name" --find clang)
clangxx=$(xcrun --sdk "$sdk_name" --find clang++)
ar=$(xcrun --sdk "$sdk_name" --find ar)
ranlib=$(xcrun --sdk "$sdk_name" --find ranlib)
build="$repo/ios/.native-build/$platform"
prefix="$build/prefix"
jobs=${NATIVE_JOBS:-8}
mkdir -p "$build" "$prefix/lib/pkgconfig"

cat > "$build/ios.toolchain.cmake" <<EOF
set(CMAKE_SYSTEM_NAME iOS)
set(CMAKE_OSX_SYSROOT $sdk_name CACHE STRING "Apple SDK" FORCE)
set(CMAKE_OSX_ARCHITECTURES arm64 CACHE STRING "Apple architecture" FORCE)
set(CMAKE_OSX_DEPLOYMENT_TARGET 17.0 CACHE STRING "Minimum iOS" FORCE)
set(CMAKE_C_COMPILER $clang CACHE FILEPATH "Apple C compiler" FORCE)
set(CMAKE_CXX_COMPILER $clangxx CACHE FILEPATH "Apple C++ compiler" FORCE)
set(CMAKE_C_COMPILER_TARGET $deployment CACHE STRING "C target" FORCE)
set(CMAKE_CXX_COMPILER_TARGET $deployment CACHE STRING "C++ target" FORCE)
set(CMAKE_FIND_ROOT_PATH $sdk;$prefix CACHE STRING "Cross-build paths" FORCE)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(ZLIB_INCLUDE_DIR $sdk/usr/include CACHE PATH "iOS zlib headers" FORCE)
set(ZLIB_LIBRARY $sdk/usr/lib/libz.tbd CACHE FILEPATH "iOS zlib" FORCE)
EOF

cat > "$prefix/lib/pkgconfig/zlib.pc" <<EOF
prefix=$sdk/usr
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: zlib
Description: iOS SDK zlib
Version: 1.3.0
Libs: -lz
Cflags: -I\${includedir}
EOF

if [[ ! -d "$build/libarchive" ]]; then
  git clone -q --depth 1 --branch v3.8.8 https://github.com/libarchive/libarchive.git "$build/libarchive"
fi
if [[ ! -f "$prefix/lib/libarchive.a" ]]; then
  cmake -S "$build/libarchive" -B "$build/libarchive-build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$build/ios.toolchain.cmake" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DBUILD_SHARED_LIBS=OFF -DENABLE_TEST=OFF -DENABLE_TAR=OFF \
    -DENABLE_CPIO=OFF -DENABLE_CAT=OFF -DENABLE_UNZIP=OFF \
    -DENABLE_OPENSSL=OFF -DENABLE_LIBB2=OFF -DENABLE_LZ4=OFF \
    -DENABLE_LZMA=OFF -DENABLE_ZSTD=OFF -DENABLE_BZip2=OFF \
    -DENABLE_LIBXML2=OFF -DENABLE_EXPAT=OFF -DENABLE_PCREPOSIX=OFF \
    -DENABLE_PCRE2POSIX=OFF -DENABLE_ICONV=OFF -DENABLE_XATTR=OFF \
    -DENABLE_ACL=OFF -DENABLE_WERROR=OFF
  cmake --build "$build/libarchive-build" --parallel "$jobs"
  cmake --install "$build/libarchive-build"
fi

if [[ ! -d "$build/ffmpeg-8.1.2" ]]; then
  curl --fail --location --silent --show-error https://ffmpeg.org/releases/ffmpeg-8.1.2.tar.xz \
    -o "$build/ffmpeg-8.1.2.tar.xz"
  tar -xf "$build/ffmpeg-8.1.2.tar.xz" -C "$build"
fi
if [[ ! -f "$prefix/lib/libavcodec.a" ]]; then
  mkdir -p "$build/ffmpeg-build"
  (
    cd "$build/ffmpeg-build"
    "$build/ffmpeg-8.1.2/configure" --prefix="$prefix" \
      --target-os=darwin --arch=arm64 --enable-cross-compile \
      --enable-static --disable-shared --disable-programs --disable-doc \
      --disable-avdevice --disable-avfilter --disable-swscale \
      --disable-autodetect --disable-symver --disable-x86asm --enable-pic \
      --cc="$clang -target $deployment -isysroot $sdk" \
      --cxx="$clangxx -target $deployment -isysroot $sdk" \
      --ld="$clang -target $deployment -isysroot $sdk" \
      --ar="$ar" --ranlib="$ranlib" \
      --extra-cflags="-miphoneos-version-min=17.0" \
      --extra-ldflags="-miphoneos-version-min=17.0"
    make -j"$jobs" install
  )
fi

export CMAKE_TOOLCHAIN_FILE="$build/ios.toolchain.cmake"
export CMAKE_GENERATOR=Ninja
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
export PKG_CONFIG_ALLOW_CROSS=1
export CARGO_BUILD_JOBS="$jobs"
export SDKROOT="$sdk"
target_variable=${target//-/_}
export "CC_${target_variable}=$clang"
export "CXX_${target_variable}=$clangxx"
export "AR_${target_variable}=$ar"
linker_variable="CARGO_TARGET_$(printf %s "$target_variable" | tr '[:lower:]' '[:upper:]')_LINKER"
export "$linker_variable=$clang"
export CFLAGS="-target $deployment -isysroot $sdk"
export CXXFLAGS="$CFLAGS"
export LDFLAGS="-target $deployment -isysroot $sdk"
rustup target add "$target"
cd "$repo"
cargo build --locked --release -p kog-ios-audio --target "$target"
echo "Built $repo/target/$target/release/libkog_ios_audio.a"
