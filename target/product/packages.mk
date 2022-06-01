# Copyright (C) 2016-2021 Paranoid Android
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

# curl
PRODUCT_PACKAGES += \
    curl

# Set compiler filter "verify" and disable AOT-compilation in dexpreopt
RELAX_USES_LIBRARY_CHECK := true

# HIDL
PRODUCT_PACKAGES += \
    android.hidl.base@1.0 \
    android.hidl.manager@1.0 \
    android.hidl.base@1.0.vendor \
    android.hidl.manager@1.0.vendor

# Neural Network
PRODUCT_PACKAGES += \
    libprotobuf-cpp-full-rtti

# Theme Picker
PRODUCT_PACKAGES += \
    ThemePicker

# Lawnchair
TARGET_BUILD_LAWNCHAIR ?= true
ifeq ($(strip $(TARGET_BUILD_LAWNCHAIR)),true)
	include vendor/lawnchair/lawnchair.mk
endif

# Matlog X
TARGET_BUILD_MATLOG ?= true
ifeq ($(strip $(TARGET_BUILD_MATLOG)),true)
PRODUCT_PACKAGES += \
    MatlogX
endif

# Graphene cam
TARGET_BUILD_GRAPHENEOS_CAMERA ?= true
ifeq ($(strip $(TARGET_BUILD_GRAPHENEOS_CAMERA)),true)
PRODUCT_PACKAGES += \
    Camera
endif

# Repainter (kdrag0n)
PRODUCT_PACKAGES += \
    RepainterServicePriv

# QTI VNDK Framework Detect
PRODUCT_PACKAGES += \
    libvndfwk_detect_jni.qti \
    libqti_vndfwk_detect \
    libqti_vndfwk_detect_system \
    libqti_vndfwk_detect_vendor \
    libvndfwk_detect_jni.qti_system \
    libvndfwk_detect_jni.qti_vendor \
    libvndfwk_detect_jni.qti.vendor \
    libqti_vndfwk_detect.vendor

# WiFi
PRODUCT_PACKAGES += \
    libwpa_client

# Updater
PRODUCT_PACKAGES += \
    Updater
