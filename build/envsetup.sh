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
                      [-j] to generate ota json for the device.
                      [-f] to generate fastboot zip
                      [-b] to generate boot.img
                      [-o | --output-dir] to set the destination dir (relative) of generated ota zip file, boot.img and such
                      [-i | --incremental] to specify directory containing incremental update zip to generate an incremental update.
                         If the directory does not contain target files then default target is built, otherwise
                         incremental target is built. New target files will be copied and replaced in this directory
                         for each build. Do note that this directory will be wiped before copying new files.
                      [--build-both-targets] to build full OTA along with an incremental OTA. Only works when [-i] is provided.
- gen_json:   Generate ota json info.
              Usage: gen_json [OPTIONS]
                      [-i] true | false. Pass in true to create json for incremental OTA.
                      [-b] true | false. Pass in true to create json for full OTA along with incremental OTA (-i must be true).
                      [-o] to set the destination dir (relative) which contains generated ota zip file.
- search:     Search in every file in the current directory for a string. Uses xargs for parallel search.
              Usage: search <string>
- reposync:   Sync repo with with some additional flags
- fetchrepos: Set up local_manifest for device and fetch the repos set in device/<vendor>/<codename>/flamingo.dependencies
              Usage: fetchrepos <device>
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

function fetchrepos() {
    if [ -z "$1" ]; then
        __print_error "Device name must not be empty"
        return 1
    fi
    if ! command -v python3 &>/dev/null; then
        __print_error "Python3 is not installed"
        return 1
    fi
    $(which python3) vendor/flamingo/build/tools/roomservice.py "$1"
}

function launch() {
    OPTIND=1
    local variant
    local wipe=false
    local installclean=false
    local json=false
    local fastbootZip=false
    local bootImage=false
    local outputDir
    local incremental=false
    local buildBothTargets=false
    local targetFilesDir
    local wipeTargetFilesDir=false

    local device="$1"
    shift # Remove device name from options

    # Check for build variant
    if ! check_variant "$1"; then
        __print_error "Invalid build variant" && return 1
    fi
    variant=$1
    shift             # Remove build variant from options
    GAPPS_BUILD=false # Reset it here everytime
    local SHORT="g,w,c,j,f,b,o:,i:"
    local LONG="gapps,wipe,output-dir:,incremental:,build-both-targets"
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
        -j)
            json=true
            shift
            ;;
        -f)
            fastbootZip=true
            shift
            ;;
        -b)
            bootImage=true
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
    if $bootImage; then
        targets+=("flamingo-boot")
    fi

    for target in "${targets[@]}"; do
        m "$target" || return 1
    done

    if [ -d "$targetFilesDir" ]; then
        if $wipeTargetFilesDir; then
            __print_info "Deleting old target files"
            rm -rf "${targetFilesDir:?}"/*
        fi
        __copy_new_target_files
    fi &&
        if $json; then
            gen_json -o "$outputDir" -i "$incremental" -b "$buildBothTargets"
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

function gen_json() {
    croot
    local GIT_BRANCH="A12.1"
    local outDir="$OUT"
    local incremental=false
    local bothTargetsExist=false

    OPTIND=1
    while getopts ":o:i:b:" option; do
        case $option in
        o) outDir="$OPTARG" ;;
        i) incremental="$OPTARG" ;;
        b) bothTargetsExist="$OPTARG" ;;
        \?)
            __print_error "Invalid option passed to gen_json, run hmm and learn the proper syntax"
            return 1
            ;;
        esac
    done

    if $bothTargetsExist && ! $incremental; then
        echo "Both targets cannot exist if not incremental"
        return 1
    fi

    if [ ! -d "$outDir" ]; then
        __print_error "Output dir $outDir doesn't exist"
        return 1
    fi

    if [ -z "$FLAMINGO_BUILD" ]; then
        __print_error "Have you run lunch?"
        return 1
    fi

    local FLAVOR
    FLAVOR=$(get_prop_value ro.flamingo.build.flavor)

    local JSON_DEVICE_DIR="ota/$FLAMINGO_BUILD/$FLAVOR"
    local JSON=$JSON_DEVICE_DIR/ota.json
    local INCREMENTAL_JSON
    if $incremental; then
        INCREMENTAL_JSON=$JSON_DEVICE_DIR/incremental_ota.json
    fi

    if [ ! -d "$JSON_DEVICE_DIR" ]; then
        mkdir -p "$JSON_DEVICE_DIR"
    fi

    local VERSION
    VERSION=$(get_prop_value ro.flamingo.build.version)

    local FILE
    FILE=$(find "$outDir" -type f -name "FlamingoOS*$FLAMINGO_BUILD*.zip" -printf "%T@ %p\n" | grep -vE "incremental|fastboot" | tail -n 1 | awk '{ print $2 }')
    if ! $incremental && [ ! -f "$FILE" ]; then
        __print_error "OTA file not found!"
        return 1
    fi
    if $incremental && $bothTargetsExist && [ ! -f "$FILE" ]; then
        __print_error "OTA file not found!"
        return 1
    fi

    local INCREMENTAL_FILE
    INCREMENTAL_FILE=$(find "$outDir" -type f -name "FlamingoOS*$FLAMINGO_BUILD*.zip" -printf "%T@ %p\n" | grep "incremental" | tail -n 1 | awk '{ print $2 }')
    if $incremental && [ ! -f "$INCREMENTAL_FILE" ]; then
        __print_error "Incremental OTA file not found!"
        return 1
    fi

    local pre_build_incremental
    if $incremental; then
        pre_build_incremental=$(unzip -o -p "$INCREMENTAL_FILE" META-INF/com/android/metadata | grep pre-build-incremental | awk -F "=" '{ print $2 }')
    fi

    local DATE
    DATE=$(($(get_prop_value ro.build.date.utc) * 1000))

    local PRIMARY_URL="https://downloads.e11z.net/d/flamingo/$GIT_BRANCH/$FLAMINGO_BUILD/$FLAVOR"

    local INCREMENTAL_NAME
    INCREMENTAL_NAME=$(basename "$INCREMENTAL_FILE")

    # Generate ota json
    if $incremental; then
        cat <<EOF >"$INCREMENTAL_JSON"
{
    "version": "$VERSION",
    "date": "$DATE",
    "download_sources": {
        "OneDrive": "$PRIMARY_URL/$INCREMENTAL_NAME"
    },
    "file_name": "$INCREMENTAL_NAME",
    "file_size": "$(du -b "$INCREMENTAL_FILE" | awk '{print $1}')",
    "sha_512": "$(sha512sum "$INCREMENTAL_FILE" | awk '{print $1}')",
    "pre_build_incremental": "$pre_build_incremental"
}
EOF
        # No need to proceed if full ota isn't present
        if ! $bothTargetsExist; then
            return 0
        fi
    fi

    local NAME
    NAME=$(basename "$FILE")
    local SIZE
    SIZE=$(du -b "$FILE" | awk '{print $1}')
    cat <<EOF >"$JSON"
{
    "version": "$VERSION",
    "date": "$DATE",
    "download_sources": {
        "OneDrive": "$PRIMARY_URL/$NAME"
    },
    "file_name": "$NAME",
    "file_size": "$SIZE",
    "sha_512": "$(sha512sum "$FILE" | awk '{print $1}')"
}
EOF
}

function get_prop_value() {
    grep "$1" "$OUT/system/build.prop" | sed "s/$1=//"
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
    local certs_dir=${ANDROID_BUILD_TOP}/certs
    [ -z "$1" ] || certs_dir=$1
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
    for key in releasekey platform shared media networkstack testkey; do
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
