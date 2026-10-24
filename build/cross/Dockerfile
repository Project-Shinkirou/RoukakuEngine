# syntax=docker/dockerfile:1.7
# SDK input is a BuildKit local named context, never an unofficial download.
FROM rust:1.98.0-trixie
ARG TARGET
ARG SDK_VERSION
ARG OSXCROSS_REV=27d21e4977c9751d01199c7a226a6faf494c3dd9
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang lld llvm cmake ninja-build pkg-config git curl ca-certificates \
    make g++ zlib1g-dev libxml2-dev libssl-dev xz-utils bzip2 patch autoconf automake libtool nasm \
    && rm -rf /var/lib/apt/lists/*
COPY --from=apple-sdk . /input-sdk/
COPY build/cross/build-toolchain.sh /tmp/build-toolchain.sh
COPY build/cross/toolchain.cmake /opt/apple/toolchain.cmake
COPY build/cross/triplets /opt/apple/triplets
COPY build/cross/vcpkg.json /opt/apple/deps/vcpkg.json
RUN bash /tmp/build-toolchain.sh "$TARGET" "$SDK_VERSION" "$OSXCROSS_REV" \
    && rm /tmp/build-toolchain.sh
ENV PATH="/opt/apple/bin:/opt/osxcross/bin:${PATH}" \
    VCPKG_OVERLAY_TRIPLETS=/opt/apple/triplets \
    PKG_CONFIG_ALLOW_CROSS=1
ENV SDKROOT=/opt/apple-sdk \
    MACOSX_DEPLOYMENT_TARGET=13.0 \
    IPHONEOS_DEPLOYMENT_TARGET=16.0 \
    CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=/opt/apple/bin/apple-cc \
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/opt/apple/bin/apple-cc \
    CARGO_TARGET_AARCH64_APPLE_IOS_LINKER=/opt/apple/bin/apple-cc \
    CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER=/opt/apple/bin/apple-cc \
    CARGO_TARGET_X86_64_APPLE_IOS_LINKER=/opt/apple/bin/apple-cc
RUN case "$TARGET" in \
      x86_64-apple-darwin) triplet=x64-osx-cross ;; \
      aarch64-apple-darwin) triplet=arm64-osx-cross ;; \
      aarch64-apple-ios) triplet=arm64-ios-cross ;; \
      aarch64-apple-ios-sim) triplet=arm64-ios-sim-cross ;; \
      x86_64-apple-ios) triplet=x64-ios-sim-cross ;; \
      *) exit 1 ;; \
    esac \
    && git init /opt/apple/vcpkg \
    && git -C /opt/apple/vcpkg remote add origin https://github.com/microsoft/vcpkg.git \
    && git -C /opt/apple/vcpkg fetch --depth=1 origin 664933b8d385dfc01f4e0cdff4c7c74e71f6a740 \
    && git -C /opt/apple/vcpkg checkout --detach FETCH_HEAD \
    && /opt/apple/vcpkg/bootstrap-vcpkg.sh -disableMetrics \
    && /opt/apple/vcpkg/vcpkg install --x-manifest-root=/opt/apple/deps \
       --x-install-root=/opt/apple/installed --overlay-triplets=/opt/apple/triplets \
       --triplet="$triplet" --clean-after-build --disable-metrics \
    && ln -s "/opt/apple/installed/$triplet" /opt/apple/native
ENV LIBASS_STATIC=1
ENV CC_x86_64_apple_darwin=/opt/apple/bin/apple-cc \
    CXX_x86_64_apple_darwin=/opt/apple/bin/apple-cxx \
    AR_x86_64_apple_darwin=/opt/apple/bin/apple-ar \
    CMAKE_TOOLCHAIN_FILE_x86_64_apple_darwin=/opt/apple/toolchain.cmake \
    PKG_CONFIG_PATH_x86_64_apple_darwin=/opt/apple/native/lib/pkgconfig \
    PKG_CONFIG_LIBDIR_x86_64_apple_darwin=/opt/apple/native/lib/pkgconfig \
    CC_aarch64_apple_darwin=/opt/apple/bin/apple-cc \
    CXX_aarch64_apple_darwin=/opt/apple/bin/apple-cxx \
    AR_aarch64_apple_darwin=/opt/apple/bin/apple-ar \
    CMAKE_TOOLCHAIN_FILE_aarch64_apple_darwin=/opt/apple/toolchain.cmake \
    PKG_CONFIG_PATH_aarch64_apple_darwin=/opt/apple/native/lib/pkgconfig \
    PKG_CONFIG_LIBDIR_aarch64_apple_darwin=/opt/apple/native/lib/pkgconfig \
    CC_aarch64_apple_ios=/opt/apple/bin/apple-cc \
    CXX_aarch64_apple_ios=/opt/apple/bin/apple-cxx \
    AR_aarch64_apple_ios=/opt/apple/bin/apple-ar \
    CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios=/opt/apple/toolchain.cmake \
    PKG_CONFIG_PATH_aarch64_apple_ios=/opt/apple/native/lib/pkgconfig \
    PKG_CONFIG_LIBDIR_aarch64_apple_ios=/opt/apple/native/lib/pkgconfig \
    CC_aarch64_apple_ios_sim=/opt/apple/bin/apple-cc \
    CXX_aarch64_apple_ios_sim=/opt/apple/bin/apple-cxx \
    AR_aarch64_apple_ios_sim=/opt/apple/bin/apple-ar \
    CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios_sim=/opt/apple/toolchain.cmake \
    PKG_CONFIG_PATH_aarch64_apple_ios_sim=/opt/apple/native/lib/pkgconfig \
    PKG_CONFIG_LIBDIR_aarch64_apple_ios_sim=/opt/apple/native/lib/pkgconfig \
    CC_x86_64_apple_ios=/opt/apple/bin/apple-cc \
    CXX_x86_64_apple_ios=/opt/apple/bin/apple-cxx \
    AR_x86_64_apple_ios=/opt/apple/bin/apple-ar \
    CMAKE_TOOLCHAIN_FILE_x86_64_apple_ios=/opt/apple/toolchain.cmake \
    PKG_CONFIG_PATH_x86_64_apple_ios=/opt/apple/native/lib/pkgconfig \
    PKG_CONFIG_LIBDIR_x86_64_apple_ios=/opt/apple/native/lib/pkgconfig
ENV BINDGEN_EXTRA_CLANG_ARGS_x86_64_apple_darwin="--target=x86_64-apple-macos13.0 -isysroot /opt/apple-sdk" \
    BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_darwin="--target=arm64-apple-macos13.0 -isysroot /opt/apple-sdk" \
    BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios="--target=arm64-apple-ios16.0 -isysroot /opt/apple-sdk" \
    BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios_sim="--target=arm64-apple-ios16.0-simulator -isysroot /opt/apple-sdk" \
    BINDGEN_EXTRA_CLANG_ARGS_x86_64_apple_ios="--target=x86_64-apple-ios16.0-simulator -isysroot /opt/apple-sdk"
