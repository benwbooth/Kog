#!/usr/bin/env bash
set -euo pipefail
# Arguments: build directory, install prefix, parallel jobs, CMake toolchain args.
# Shared by iOS and Android; every compression backend is built for the target.
build=$1 prefix=$2 jobs=$3
shift 3
repo=$(cd "$(dirname "$0")/.." && pwd)
cmake -S "$repo/native/archive-deps" -B "$build/archive-deps" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_LIBDIR=lib "$@"
cmake --build "$build/archive-deps" --parallel "$jobs"
cmake --install "$build/archive-deps"

# Correct the upstream pkg-config names after renaming the static libraries.
for entry in 'liblzma kog_liblzma lzma' 'libzstd kog_zstd zstd'; do
  read -r module name upstream <<< "$entry"
  sed "s/-l$upstream/-l$name/g" "$prefix/lib/pkgconfig/$module.pc" > "$build/$module.pc"
  cp "$build/$module.pc" "$prefix/lib/pkgconfig/$module.pc"
done

if [[ ! -d "$build/libarchive" ]]; then
  git clone -q --depth 1 --branch v3.8.8 https://github.com/libarchive/libarchive.git "$build/libarchive"
fi
# Reconfigure on every run: an old archive without these codecs must never
# satisfy the cache merely because libarchive.a already exists.
PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" cmake \
  -S "$build/libarchive" -B "$build/libarchive-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_LIBDIR=lib -DCMAKE_PREFIX_PATH="$prefix" \
  -DBUILD_SHARED_LIBS=OFF -DENABLE_TEST=OFF -DENABLE_TAR=OFF \
  -DENABLE_CPIO=OFF -DENABLE_CAT=OFF -DENABLE_UNZIP=OFF \
  -DENABLE_OPENSSL=OFF -DENABLE_LIBB2=OFF \
  -DENABLE_LZ4=ON -DLZ4_INCLUDE_DIR="$prefix/include" -DLZ4_LIBRARY="$prefix/lib/liblz4.a" \
  -DENABLE_LZMA=ON -DLIBLZMA_INCLUDE_DIR="$prefix/include" -DLIBLZMA_LIBRARY="$prefix/lib/libkog_liblzma.a" \
  -DENABLE_ZSTD=ON -DZSTD_INCLUDE_DIR="$prefix/include" -DZSTD_LIBRARY="$prefix/lib/libkog_zstd.a" \
  -DENABLE_BZip2=ON -DBZIP2_INCLUDE_DIR="$prefix/include" -DBZIP2_LIBRARIES="$prefix/lib/libbz2.a" \
  -DENABLE_LIBXML2=OFF -DENABLE_EXPAT=OFF -DENABLE_PCREPOSIX=OFF \
  -DENABLE_PCRE2POSIX=OFF -DENABLE_ICONV=OFF -DENABLE_XATTR=OFF \
  -DENABLE_ACL=OFF -DENABLE_WERROR=OFF "$@"
for feature in HAVE_LIBLZMA HAVE_LIBZSTD HAVE_LIBLZ4 HAVE_LIBBZ2; do
  if ! grep -q "^#define $feature 1" "$build/libarchive-build/config.h"; then
    echo "libarchive is missing required compression backend: $feature" >&2
    exit 1
  fi
done
cmake --build "$build/libarchive-build" --parallel "$jobs"
cmake --install "$build/libarchive-build"
