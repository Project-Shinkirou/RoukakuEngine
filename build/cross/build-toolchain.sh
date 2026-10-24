#!/usr/bin/env bash
set -euo pipefail
target=$1
version=$2
revision=$3
test -f /input-sdk/SDKSettings.json
mkdir -p /opt/apple/bin
case "$target" in
  x86_64-apple-darwin|aarch64-apple-darwin)
    git init /tmp/osxcross
    git -C /tmp/osxcross remote add origin https://github.com/tpoechtrager/osxcross.git
    git -C /tmp/osxcross fetch --depth=1 origin "$revision"
    git -C /tmp/osxcross checkout --detach FETCH_HEAD
    mv /input-sdk "/tmp/MacOSX${version}.sdk"
    tar -C /tmp -cJf "/tmp/osxcross/tarballs/MacOSX${version}.sdk.tar.xz" "MacOSX${version}.sdk"
    (cd /tmp/osxcross && UNATTENDED=1 BUILD_FLAVOR=llvm ENABLE_ARCHS="arm64 x86_64" \
      OSX_VERSION_MIN=13.0 TARGET_DIR=/opt/osxcross ./build.sh)
    ln -s /opt/osxcross/SDK/MacOSX*.sdk /opt/apple-sdk
    if [[ $target == aarch64-* ]]; then cc=oa64-clang; else cc=o64-clang; fi
    printf '#!/bin/sh\nexec /opt/osxcross/bin/%s "$@"\n' "$cc" > /opt/apple/bin/apple-cc
    printf '#!/bin/sh\nexec /opt/osxcross/bin/%s++ "$@"\n' "$cc" > /opt/apple/bin/apple-cxx
    rm -rf /tmp/osxcross "/tmp/MacOSX${version}.sdk"
    ;;
  aarch64-apple-ios) clang_target=arm64-apple-ios16.0; sdk_name=iPhoneOS.sdk ;;
  aarch64-apple-ios-sim) clang_target=arm64-apple-ios16.0-simulator; sdk_name=iPhoneSimulator.sdk ;;
  x86_64-apple-ios) clang_target=x86_64-apple-ios16.0-simulator; sdk_name=iPhoneSimulator.sdk ;;
  *) echo "Unsupported Apple triple: $target" >&2; exit 1 ;;
esac
if [[ $target != *darwin ]]; then
  mkdir -p /opt/apple/SDKs
  mv /input-sdk "/opt/apple/SDKs/$sdk_name"
  ln -s "/opt/apple/SDKs/$sdk_name" /opt/apple-sdk
  printf '#!/bin/sh\nexec clang --target=%s -isysroot /opt/apple-sdk -fuse-ld=lld "$@"\n' "$clang_target" > /opt/apple/bin/apple-cc
  printf '#!/bin/sh\nexec clang++ --target=%s -isysroot /opt/apple-sdk -stdlib=libc++ -fuse-ld=lld "$@"\n' "$clang_target" > /opt/apple/bin/apple-cxx
fi
ln -s /usr/bin/llvm-ar /opt/apple/bin/apple-ar
chmod 755 /opt/apple/bin/apple-cc /opt/apple/bin/apple-cxx
printf '%s\n' "$target" > /opt/apple/target
