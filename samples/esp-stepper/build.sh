#!/bin/bash
set -euo pipefail

BUILD_MODE="release"
FLASH=false
MODEL="esp32"

declare -A DEVICE_PROFILES=(
    ["esp32"]="esp:xtensa-esp32-none-elf:web-flash"
    ["esp32c2"]="nightly:riscv32imc-unknown-none-elf:web-flash"
    ["esp32c3"]="nightly:riscv32imc-unknown-none-elf:web-flash"
    ["esp32c6"]="nightly:riscv32imac-unknown-none-elf:web-flash"
    ["esp32h2"]="nightly:riscv32imac-unknown-none-elf:web-flash"
    ["esp32s2"]="esp:xtensa-esp32s2-none-elf:web-flash"
    ["esp32s3"]="esp:xtensa-esp32s3-none-elf:web-flash"
)

while [ $# -gt 0 ]; do
    case "$1" in
        "release" | "debug")
            BUILD_MODE="$1"
            shift
            ;;
        "-f" | "--flash")
            FLASH=true
            shift
            ;;
        "-m" | "--model")
            if [ $# -lt 2 ]; then
                echo "Error: --model requires an argument" >&2
                exit 1
            fi
            MODEL="$2"
            shift 2
            ;;
        *)
            echo "Usage: $0 [debug|release] [--flash] [--model <MODEL>]"
            echo "Supported models: std ${!DEVICE_PROFILES[*]}"
            exit 1
            ;;
    esac
done

resolve_device_config() {
    local model=$1

    if [[ -v DEVICE_PROFILES["$model"] ]]; then
        IFS=':' read -ra config <<< "${DEVICE_PROFILES[$model]}"
        echo "+${config[0]} ${config[1]} ${config[2]}"
    else
        echo "Error: Unsupported model '$model'. Supported: ${!DEVICE_PROFILES[*]}" >&2
        exit 1
    fi
}

read -r EMBEDDED_TOOLCHAIN EMBEDDED_TARGET FLASH_TOOL <<< $(resolve_device_config "$MODEL")

build_device() {
    local args=()

    [[ -n "$EMBEDDED_TARGET" ]] && args+=(--target "$EMBEDDED_TARGET")
    [[ "$BUILD_MODE" == "release" ]] && args+=(--release)
    args+=(--features "$MODEL")

    echo "Building embedded for $MODEL in $BUILD_MODE mode..."
    (
        if [[ "$MODEL" == esp* ]]; then
            if ! command -v idf.py &>/dev/null; then
                echo "Loading ESP environment..."
                export MCU="$MODEL"
                source ~/export-esp.sh >/dev/null 2>&1
            fi
        fi

        set -e
        cargo $EMBEDDED_TOOLCHAIN build "${args[@]}"
    ) || exit 1
}

flash_device() {
    local flash_cmd=""

    case "$MODEL" in
        esp*)
            flash_cmd="$FLASH_TOOL --chip $MODEL target/$EMBEDDED_TARGET/$BUILD_MODE/esp-stepper"
            ;;
        *)
            echo "Error: Unsupported model '$MODEL' for flashing." >&2
            exit 1
            ;;
    esac

    echo "Flashing with: $flash_cmd"
    $flash_cmd
}

# Main
build_device

if $FLASH; then
    flash_device
fi
