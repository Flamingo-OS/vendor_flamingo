# Copyright (C) 2021 Paranoid Android
# Copyright (C) 2022 FlamingoOS Project
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

ifneq ($(wildcard certs/releasekey.*),)
SIGNING_KEYS := certs/releasekey
endif

ifeq ($(strip $(OFFICIAL_BUILD)),true)
    ifeq ($(strip $(SIGNING_KEYS)),)
        $(error "Official builds must be signed with release keys, run keygen to generate signing keys")
    endif
endif

SIGNING_KEYS ?= $(DEFAULT_KEY_CERT_PAIR)

PRODUCT_DEFAULT_DEV_CERTIFICATE := $(SIGNING_KEYS)
PRODUCT_OTA_PUBLIC_KEYS := $(PRODUCT_DEFAULT_DEV_CERTIFICATE)

# Versioning.
$(call inherit-product, vendor/flamingo/target/product/version.mk)

# Don't dexpreopt prebuilts. (For GMS).
DONT_DEXPREOPT_PREBUILTS := true

# Filesystem
TARGET_FS_CONFIG_GEN += vendor/flamingo/target/config/config.fs

# Include Common Qualcomm Device Tree.
$(call inherit-product, device/qcom/common/common.mk)

# Include definitions for Snapdragon Clang
$(call inherit-product, vendor/qcom/sdclang/config/SnapdragonClang.mk)

# Include Overlay makefile.
$(call inherit-product, vendor/flamingo/overlay/overlays.mk)

# Include Packages makefile.
$(call inherit-product, vendor/flamingo/target/product/packages.mk)

# Include Properties makefile.
$(call inherit-product, vendor/flamingo/target/product/properties.mk)

ifeq ($(GAPPS_BUILD),true)
    # Include GMS, Modules, and Pixel features.
    $(call inherit-product-if-exists, vendor/google/gms/config.mk)
    $(call inherit-product-if-exists, vendor/google/pixel/config.mk)
endif

# Flatten APEXs for performance
OVERRIDE_TARGET_FLATTEN_APEX := true
# This needs to be specified explicitly to override ro.apex.updatable=true from
# # prebuilt vendors, as init reads /product/build.prop after /vendor/build.prop
PRODUCT_PRODUCT_PROPERTIES += ro.apex.updatable=false

# Move Wi-Fi modules to vendor.
PRODUCT_VENDOR_MOVE_ENABLED := true

# Permissions
PRODUCT_COPY_FILES += \
    vendor/flamingo/target/config/permissions/privapp-permissions-qti.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/permissions/privapp-permissions-qti.xml \
    vendor/flamingo/target/config/permissions/privapp-permissions-hotword.xml:$(TARGET_COPY_OUT_PRODUCT)/etc/permissions/privapp-permissions-hotword.xml \
    vendor/flamingo/target/config/permissions/qti_whitelist.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/sysconfig/qti_whitelist.xml

# Sensitive phone numbers and APN configurations
PRODUCT_COPY_FILES += \
    vendor/flamingo/target/config/apns-conf.xml:$(TARGET_COPY_OUT_PRODUCT)/etc/apns-conf.xml \
    vendor/flamingo/target/config/sensitive_pn.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/sensitive_pn.xml

# Skip boot JAR checks.
SKIP_BOOT_JARS_CHECK := true

# Strip the local variable table and the local variable type table to reduce
# the size of the system image. This has no bearing on stack traces, but will
# leave less information available via JDWP.
PRODUCT_MINIMIZE_JAVA_DEBUG_INFO := true

# Sepolicy
$(call inherit-product, vendor/flamingo/target/product/sepolicy.mk)

# Privapp permissions
PRODUCT_PACKAGES += \
    privapp_additional_whitelist_com.android.systemui \
    privapp_additional_whitelist_com.android.settings

# Theme overlays
$(call inherit-product, vendor/themes/common.mk)

# Inherit from lineage hidl repo
$(call inherit-product-if-exists, hardware/lineage/interfaces/config.mk)

# BootAnimation
include vendor/flamingo/target/config/bootanimation.mk
