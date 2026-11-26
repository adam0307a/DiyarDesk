#!/usr/bin/env bash
set -euo pipefail

# Cross-compile native/diyardesk-gl (Rust + OpenGL ES 3.0) into Android jniLibs.
#
# Usage:
#   ./android/build-native-gl.sh
#   ANDROID_ABIS="arm64-v8a" ./android/build-native-gl.sh
#
# Requirements:
#   - rustup with Android targets (aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android)
#   - Android NDK (ANDROID_NDK_HOME or ANDROID_NDK_ROOT or $ANDROID_SDK_ROOT/ndk/*)
#   - Optional: cargo-ndk (preferred; falls back to direct NDK clang linker if absent)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATE_DIR="${REPO_ROOT}/native/diyardesk-gl"
JNILIBS_DIR="${SCRIPT_DIR}/app/src/main/jniLibs"
MIN_SDK="${ANDROID_MIN_SDK:-29}"
ABIS="${ANDROID_ABIS:-arm64-v8a armeabi-v7a x86_64}"

find_ndk() {
  if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
    echo "${ANDROID_NDK_HOME}"
    return 0
  fi
  if [[ -n "${ANDROID_NDK_LATEST_HOME:-}" && -d "${ANDROID_NDK_LATEST_HOME}" ]]; then
    echo "${ANDROID_NDK_LATEST_HOME}"
    return 0
  fi
  if [[ -n "${ANDROID_NDK_ROOT:-}" && -d "${ANDROID_NDK_ROOT}" ]]; then
    echo "${ANDROID_NDK_ROOT}"
    return 0
  fi
  local sdk="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
  if [[ -d "${sdk}/ndk" ]]; then
    local latest
    latest="$(ls -1 "${sdk}/ndk" | sort -V | tail -n 1 || true)"
    if [[ -n "${latest}" && -d "${sdk}/ndk/${latest}" ]]; then
      echo "${sdk}/ndk/${latest}"
      return 0
    fi
  fi
  return 1
}

NDK_DIR="$(find_ndk || true)"
if [[ -z "${NDK_DIR}" ]]; then
  echo "ERROR: Android NDK not found. Set ANDROID_NDK_HOME or install NDK via sdkmanager." >&2
  exit 1
fi
export ANDROID_NDK_HOME="${NDK_DIR}"
echo "Using Android NDK: ${ANDROID_NDK_HOME}"
echo "Building native Rust + OpenGL ES 3.0 libraries for ABIs: ${ABIS} (API ${MIN_SDK})"

mkdir -p "${JNILIBS_DIR}"

if command -v cargo-ndk >/dev/null 2>&1; then
  TARGET_ARGS=()
  for abi in ${ABIS}; do
    TARGET_ARGS+=(-t "${abi}")
  done
  (
    cd "${CRATE_DIR}"
    CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384" \
    CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384" \
      cargo ndk "${TARGET_ARGS[@]}" --platform "${MIN_SDK}" -o "${JNILIBS_DIR}" build --release
  )
else
  HOST_TAG="linux-x86_64"
  case "$(uname -s)" in
    Darwin) HOST_TAG="darwin-x86_64" ;;
  esac
  TOOLCHAIN_BIN="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/${HOST_TAG}/bin"
  if [[ ! -d "${TOOLCHAIN_BIN}" ]]; then
    echo "ERROR: NDK LLVM toolchain not found at ${TOOLCHAIN_BIN}" >&2
    exit 1
  fi

  for abi in ${ABIS}; do
    EXTRA_RUSTFLAGS=""
    case "${abi}" in
      arm64-v8a)
        RUST_TARGET="aarch64-linux-android"
        CLANG_PREFIX="aarch64-linux-android${MIN_SDK}"
        TARGET_LOWER="aarch64_linux_android"
        TARGET_UPPER="AARCH64_LINUX_ANDROID"
        EXTRA_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
        ;;
      armeabi-v7a)
        RUST_TARGET="armv7-linux-androideabi"
        CLANG_PREFIX="armv7a-linux-androideabi${MIN_SDK}"
        TARGET_LOWER="armv7_linux_androideabi"
        TARGET_UPPER="ARMV7_LINUX_ANDROIDEABI"
        ;;
      x86_64)
        RUST_TARGET="x86_64-linux-android"
        CLANG_PREFIX="x86_64-linux-android${MIN_SDK}"
        TARGET_LOWER="x86_64_linux_android"
        TARGET_UPPER="X86_64_LINUX_ANDROID"
        EXTRA_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
        ;;
      *)
        echo "ERROR: Unsupported ABI '${abi}'" >&2
        exit 1
        ;;
    esac

    LINKER="${TOOLCHAIN_BIN}/${CLANG_PREFIX}-clang"
    AR="${TOOLCHAIN_BIN}/llvm-ar"
    if [[ ! -x "${LINKER}" ]]; then
      echo "ERROR: NDK linker not found: ${LINKER}" >&2
      exit 1
    fi

    echo "Compiling ${RUST_TARGET} (${abi}) via ${LINKER}..."
    (
      cd "${CRATE_DIR}"
      ENV_ARGS=(
        "CARGO_TARGET_${TARGET_UPPER}_LINKER=${LINKER}"
        "CC_${TARGET_LOWER}=${LINKER}"
        "AR_${TARGET_LOWER}=${AR}"
      )
      if [[ -n "${EXTRA_RUSTFLAGS}" ]]; then
        ENV_ARGS+=("CARGO_TARGET_${TARGET_UPPER}_RUSTFLAGS=${EXTRA_RUSTFLAGS}")
      fi
      env "${ENV_ARGS[@]}" cargo build --release --target "${RUST_TARGET}"
    )

    mkdir -p "${JNILIBS_DIR}/${abi}"
    cp "${CRATE_DIR}/target/${RUST_TARGET}/release/libdiyardesk_gl.so" \
      "${JNILIBS_DIR}/${abi}/libdiyardesk_gl.so"
  done
fi

echo "Native Android OpenGL ES 3.0 libraries ready:"
ls -lh "${JNILIBS_DIR}"/*/libdiyardesk_gl.so
