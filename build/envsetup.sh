#!/bin/bash

# Copyright 2022 FlamingoOS Project
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Clear the screen
clear

# Colors
LR="\033[1;31m"
LG="\033[1;32m"
LY="\033[1;33m"
NC="\033[0m"

# Common tags
ERROR="${LR}Error"
INFO="${LG}Info"
WARN="${LY}Warning"

# Set to non gapps build by default
export GAPPS_BUILD=false

export SKIP_ABI_CHECKS="true"
export TEMPORARY_DISABLE_PATH_RESTRICTIONS=true

function flamingo_help() {
    cat <<EOF
Flamingo OS specific functions:
- launch:     Build a full ota package.
              Usage: launch <device> <variant> [OPTIONS]
                      [-g | --gapps] to build gapps variant.
                      [-w | --wipe] to wipe out directory.
                      [-c] to do an install-clean. Also deletes contents of target files dir before copying new target
                         files if specified with -i option
                      [-f] to generate fastboot zip
                      [--images] IMG1,IMG2.. to copy out specified images from generated target files.
                      [-o | --output-dir] to set the destination dir (relative) of generated ota zip file, boot.img and such
                      [-i | --incremental] to specify directory containing incremental update zip to generate an incremental update.
                         If the directory does not contain target files then default target is built, otherwise
                         incremental target is built. New target files will be copied and replaced in this directory
                         for each build. Do note that this directory will be wiped before copying new files.
                      [--build-both-targets] to build full OTA along with an incremental OTA. Only works when [-i] is provided.
- search:     Search in every file in the current directory for a string. Uses xargs for parallel search.
              Usage: search <string>
- reposync:   Sync repo with with some additional flags
- roomservice: Set up local_manifest for device and fetch the repos set in device/<vendor>/<codename>/flamingo.dependencies
               Run roomservice --help for more info.
- mergecaf: Merge in a newer caf tag across the source
              usage: mergecaf <caf tag>
- keygen:     Generate keys for signing builds.
              Usage: keygen <dir>
              Default output dir is ${ANDROID_BUILD_TOP}/certs
- sideload:   Sideload a zip while device is booted. It will boot to recovery, sideload the file and boot you back to system
              Usage: sideload [FILE]
EOF
}

function __timer() {
    local time=$(($2 - $1))
    local sec=$((time % 60))
    local min=$((time / 60))
    local hr=$((min / 60))
    local min=$((min % 60))
    echo "$hr:$min:$sec"
}

function roomservice() {
    ./vendor/flamingo/scripts/roomservice/target/release/roomservice --manifest-root ".repo" $*
}

function mergecaf() {
    cargo run -qr --manifest-path vendor/flamingo/scripts/manifest_merger/Cargo.toml -- $*
}

function launch() {
    OPTIND=1
    local variant
    local wipe=false
    local installclean=false
    local fastbootZip=false
    local outputDir
    local incremental=false
    local buildBothTargets=false
    local targetFilesDir
    local wipeTargetFilesDir=false
    local images=()

    local device="$1"
    shift # Remove device name from options

    # Check for build variant
    if ! check_variant "$1"; then
        __print_error "Invalid build variant" && return 1
    fi
    variant=$1
    shift             # Remove build variant from options
    GAPPS_BUILD=false # Reset it here everytime
    local SHORT="g,w,c,f,o:,i:"
    local LONG="gapps,wipe,output-dir:,incremental:,build-both-targets,images:"
    local OPTS
    if ! OPTS=$(getopt -a -n launch --options $SHORT --longoptions $LONG -- "$@"); then
        return 1
    fi

    eval set -- "$OPTS"

    while :; do
        case "$1" in
        -g)
            export GAPPS_BUILD=true
            shift
            ;;
        -w)
            wipe=true
            wipeTargetFilesDir=true
            shift
            ;;
        -c)
            installclean=true
            wipeTargetFilesDir=true
            shift
            ;;
        -f)
            fastbootZip=true
            shift
            ;;
        -o | --output-dir)
            outputDir="$2"
            shift 2
            ;;
        -i | --incremental)
            targetFilesDir="$2"
            shift 2
            ;;
        --build-both-targets)
            buildBothTargets=true
            shift
            ;;
        --images)
            IFS="," read -a images <<< "$2"
            shift 2
            ;;
        --)
            shift
            break
            ;;
        *)
            __print_error "Unknown option: $1"
            return 1
            ;;
        esac
    done

    if $buildBothTargets && [[ -z "$targetFilesDir" ]] ; then
        __print_error "--build-both-targets should not be used without --incremental | -i"
        return 1
    fi

    # Execute rest of the commands now as all vars are set.
    startTime=$(date "+%s")

    if ! lunch "flamingo_$device-$variant"; then
        return 1
    fi

    if $wipe; then
        make clean
        [ -d "$outputDir" ] && rm -rf "${outputDir:?}/*"
    elif $installclean; then
        make install-clean
        rm -rf "$OUT/obj/KERNEL_OBJ"
        [ -d "$outputDir" ] && rm -rf "${outputDir:?}/*"
    fi

    if [ -z "$outputDir" ]; then
        outputDir="$OUT"
    else
        outputDir="$ANDROID_BUILD_TOP/$outputDir"
        [ -d "$outputDir" ] || mkdir -p "$outputDir"
    fi

    export FLAMINGO_OUT="$outputDir"

    if [ -n "$targetFilesDir" ]; then
        if $wipeTargetFilesDir; then
            __print_warn "All files in $targetFilesDir will be deleted before copying new target files"
        fi
        if [ ! -d "$targetFilesDir" ]; then
            mkdir -p "$targetFilesDir"
        fi
    fi

    local targets=("flamingo")
    local previousTargetFile
    if [ -n "$targetFilesDir" ]; then
        previousTargetFile=$(find "$targetFilesDir" -type f -name "*target_files*.zip" | sort -n | tail -n 1)
        if [ -n "$previousTargetFile" ]; then
            incremental=true
            if $buildBothTargets; then
                targets+=("flamingo-incremental")
            else
                targets=("flamingo-incremental")
            fi
            export PREVIOUS_TARGET_FILES_PACKAGE="$previousTargetFile"
        else
            __print_info "Previous target files package not present, using default target"
        fi
    fi
    if ! $incremental ; then
        if [ -n "$PREVIOUS_TARGET_FILES_PACKAGE" ]; then
            export PREVIOUS_TARGET_FILES_PACKAGE=
        fi
        # Reset if previous target files is not present
        if $buildBothTargets ; then
            buildBothTargets=false
        fi
    fi
    if $fastbootZip; then
        targets+=("flamingo-fastboot")
    fi

    for target in "${targets[@]}"; do
        m "$target" || return 1
    done

    local export_dir=$(get_build_var FLAMINGO_OUT)
    local img_prefix=$(get_build_var FLAMINGO_OTA_PACKAGE_NAME)
    local intermediates_dir="$OUT/obj/PACKAGING/target_files_intermediates"
    for img in "${images[@]}"; do
        cp -f "$intermediates_dir"/*/IMAGES/"$img".img "$export_dir/$img_prefix-$(date +%Y%m%d-%H%M)-$img.img" || return 1
    done

    if [ -d "$targetFilesDir" ]; then
        if $wipeTargetFilesDir; then
            __print_info "Deleting old target files"
            rm -rf "${targetFilesDir:?}"/*
        fi
        __copy_new_target_files
    fi
    local STATUS=$?

    endTime=$(date "+%s")
    __print_info "Build finished in $(__timer "$startTime" "$endTime")"

    if [ $STATUS -ne 0 ]; then
        return $STATUS
    fi
}

function __zip_append_timestamp() {
    local TIME
    TIME=$(date "+%Y%m%d-%H%M")
    local APPENDED_ZIP
    APPENDED_ZIP=$(sed -r "s/-*[0-9]*-*[0-9]*.zip//" <<<"$1")-"$TIME.zip"
    echo "$APPENDED_ZIP"
}

function __copy_new_target_files() {
    local newTargetFile
    newTargetFile=$(find "$OUT" -type f -name "*target_files*.zip" -print -quit)
    if [ -z "$newTargetFile" ]; then
        return 1
    fi
    local destTargetFile
    destTargetFile=$(basename "$newTargetFile")
    destTargetFile=$(__zip_append_timestamp "$destTargetFile")
    __print_info "Copying new target files package"
    cp "$newTargetFile" "$targetFilesDir/$destTargetFile"
}

function search() {
    [ -z "$1" ] && echo -e "${ERROR}: provide a string to search${NC}" && return 1
    find . -type f -print0 | xargs -0 -P "$(nproc --all)" grep "$*" && return 0
}

function reposync() {
    repo sync -j"$(nproc --all)" --optimized-fetch --no-clone-bundle --no-tags --current-branch "$@"
    return $?
}

function keygen() {
    local certs_dir="${ANDROID_BUILD_TOP}/certs"
    [ -z "$1" ] || certs_dir="$1"
    rm -rf "$certs_dir"
    mkdir -p "$certs_dir"
    local subject
    echo "Sample subject: '/C=US/ST=California/L=Mountain View/O=Android/OU=Android/CN=Android/emailAddress=android@android.com'"
    echo "Now enter subject details for your keys:"
    for entry in C ST L O OU CN emailAddress; do
        echo -n "$entry:"
        read -r val
        subject+="/$entry=$val"
    done
    local keys=( releasekey platform shared media networkstack testkey sdk_sandbox bluetooth )
    for key in ${keys[@]}; do
        ./development/tools/make_key "$certs_dir"/$key "$subject"
    done
}

function sideload() {
    adb wait-for-device reboot sideload-auto-reboot && adb wait-for-device-sideload && adb sideload "$1"
}

function __print_info() {
    echo -e "${INFO}: $*${NC}"
}

function __print_warn() {
    echo -e "${WARN}: $*${NC}"
}

function __print_error() {
    echo -e "${ERROR}: $*${NC}"
}
