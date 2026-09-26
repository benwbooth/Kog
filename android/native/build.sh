#!/usr/bin/env bash
set -euo pipefail

# Build Kog's native decoder and its third-party libraries for one Android ABI.
# Run before Gradle so app/src/main/jniLibs contains the output for packaging.
abi=${1:-arm64-v8a}
case "$abi" in
  arm64-v8a) target=aarch64-linux-android; arch=aarch64; lib_triple=aarch64-linux-android ;;
  x86_64) target=x86_64-linux-android; arch=x86_64; lib_triple=x86_64-linux-android ;;
  *) echo "Unsupported Android ABI: $abi" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "$0")/../.." && pwd)
sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
ndk=${ANDROID_NDK_HOME:-${sdk:+$sdk/ndk/28.2.13676358}}
if [[ -z "$ndk" || ! -f "$ndk/build/cmake/android.toolchain.cmake" ]]; then
  echo "Install Android NDK 28.2.13676358 or set ANDROID_NDK_HOME" >&2
  exit 2
fi
for command in cmake ninja pkg-config curl git make; do
  command -v "$command" >/dev/null || { echo "Missing $command" >&2; exit 2; }
done

build="$repo/android/.native-build/$abi"
prefix="$build/prefix"
toolchain="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
sysroot="$ndk/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
zlib_library="$sysroot/usr/lib/$lib_triple/28/libz.so"
mkdir -p "$build" "$prefix/lib/pkgconfig"

if [[ ! -d "$build/libarchive" ]]; then
  git clone -q --depth 1 --branch v3.8.8 https://github.com/libarchive/libarchive.git "$build/libarchive"
fi
if [[ ! -f "$prefix/lib/libarchive.a" ]]; then
  cmake -S "$build/libarchive" -B "$build/libarchive-build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$ndk/build/cmake/android.toolchain.cmake" \
    -DANDROID_ABI="$abi" -DANDROID_PLATFORM=android-28 \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DZLIB_INCLUDE_DIR="$sysroot/usr/include" -DZLIB_LIBRARY="$zlib_library" \
    -DBUILD_SHARED_LIBS=OFF -DENABLE_TEST=OFF -DENABLE_TAR=OFF \
    -DENABLE_CPIO=OFF -DENABLE_CAT=OFF -DENABLE_UNZIP=OFF \
    -DENABLE_OPENSSL=OFF -DENABLE_LIBB2=OFF -DENABLE_LZ4=OFF \
    -DENABLE_LZMA=OFF -DENABLE_ZSTD=OFF -DENABLE_BZip2=OFF \
    -DENABLE_LIBXML2=OFF -DENABLE_EXPAT=OFF -DENABLE_PCREPOSIX=OFF \
    -DENABLE_PCRE2POSIX=OFF -DENABLE_ICONV=OFF -DENABLE_XATTR=OFF \
    -DENABLE_ACL=OFF -DENABLE_WERROR=OFF
  cmake --build "$build/libarchive-build" --parallel "${NATIVE_JOBS:-8}"
  cmake --install "$build/libarchive-build"
fi

cat > "$prefix/lib/pkgconfig/zlib.pc" <<EOF
prefix=$sysroot/usr
libdir=\${prefix}/lib/$lib_triple/28
includedir=\${prefix}/include
Name: zlib
Description: Android platform zlib
Version: 1.3.0
Libs: -L\${libdir} -lz
Cflags: -I\${includedir}
EOF

if [[ ! -d "$build/ffmpeg-8.1.2" ]]; then
  curl --fail --location --silent --show-error \
    https://ffmpeg.org/releases/ffmpeg-8.1.2.tar.xz \
    -o "$build/ffmpeg-8.1.2.tar.xz"
  tar -xf "$build/ffmpeg-8.1.2.tar.xz" -C "$build"
fi
if [[ ! -f "$prefix/lib/libavcodec.a" ]]; then
  mkdir -p "$build/ffmpeg-build"
  (
    cd "$build/ffmpeg-build"
    "$build/ffmpeg-8.1.2/configure" --prefix="$prefix" \
      --target-os=android --arch="$arch" --enable-cross-compile \
      --enable-static --disable-shared --disable-programs --disable-doc \
      --disable-avdevice --disable-avfilter --disable-swscale \
      --disable-autodetect --disable-symver --disable-x86asm --enable-pic \
      --cc="$toolchain/${target}28-clang" \
      --cxx="$toolchain/${target}28-clang++" \
      --ld="$toolchain/${target}28-clang" \
      --ar="$toolchain/llvm-ar" --ranlib="$toolchain/llvm-ranlib" \
      --nm="$toolchain/llvm-nm" --strip="$toolchain/llvm-strip"
    make -j"${NATIVE_JOBS:-8}" install
  )
fi

cat > "$build/android.toolchain.cmake" <<EOF
set(ANDROID_ABI $abi CACHE STRING "Android target ABI" FORCE)
set(ANDROID_PLATFORM android-28 CACHE STRING "Android target API" FORCE)
include($ndk/build/cmake/android.toolchain.cmake)
set(ZLIB_INCLUDE_DIR $sysroot/usr/include CACHE PATH "Android zlib headers" FORCE)
set(ZLIB_LIBRARY $zlib_library CACHE FILEPATH "Android zlib library" FORCE)
EOF

export ANDROID_NDK_HOME="$ndk"
export CMAKE_TOOLCHAIN_FILE="$build/android.toolchain.cmake"
export CMAKE_GENERATOR=Ninja
export ANDROID_ABI="$abi" ANDROID_PLATFORM=android-28
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig" PKG_CONFIG_ALLOW_CROSS=1
export CARGO_BUILD_JOBS="${NATIVE_JOBS:-8}"
target_variable=${target//-/_}
export "CC_${target_variable}=$toolchain/${target}28-clang"
export "CXX_${target_variable}=$toolchain/${target}28-clang++"
export "AR_${target_variable}=$toolchain/llvm-ar"
export "CARGO_TARGET_${target_variable^^}_LINKER=$toolchain/${target}28-clang"
rustup target add "$target"
cd "$repo"
cargo build --locked -p kog-android-audio --target "$target"

jni="$repo/android/app/src/main/jniLibs/$abi"
mkdir -p "$jni"
cp "$repo/target/$target/debug/libkog_android_audio.so" "$jni/"
"$toolchain/llvm-strip" --strip-unneeded "$jni/libkog_android_audio.so"
cp "$sysroot/usr/lib/$lib_triple/libc++_shared.so" "$jni/"
"$toolchain/llvm-strip" --strip-unneeded "$jni/libc++_shared.so"
for helper in kog-sfm-helper kog-psf-helper kog-2sf-helper \
              kog-snsf-helper kog-sc55-helper; do
  binary=$(find "$repo/target/$target/debug/build" -type f -path "*/bin/$helper" -print -quit)
  if [[ -z "$binary" ]]; then
    echo "Kog did not build $helper for $abi" >&2
    exit 1
  fi
  cp "$binary" "$jni/lib$helper.so"
  "$toolchain/llvm-strip" --strip-unneeded "$jni/lib$helper.so"
done
echo "Installed Kog's Rust decoder and helpers in $jni"
