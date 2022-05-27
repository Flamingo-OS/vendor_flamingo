# Copyright 2022 Flamingo
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

# Version and fingerprint
FLAMINGO_VERSION_MAJOR := 1
FLAMINGO_VERSION_MINOR := 0
FLAMINGO_VERSION := $(FLAMINGO_VERSION_MAJOR).$(FLAMINGO_VERSION_MINOR)

# Set props
PRODUCT_SYSTEM_DEFAULT_PROPERTIES += \
  ro.flamingo.build.device=$(FLAMINGO_BUILD) \
  ro.flamingo.build.version=$(FLAMINGO_VERSION)

FLAMINGO_OTA_PACKAGE_NAME := FLAMINGO-$(FLAMINGO_VERSION)-$(FLAMINGO_BUILD)-$(TARGET_BUILD_VARIANT)

ifeq ($(strip $(OFFICIAL_BUILD)),true)
FLAMINGO_OTA_PACKAGE_NAME := $(FLAMINGO_OTA_PACKAGE_NAME)-OFFICIAL
else
FLAMINGO_OTA_PACKAGE_NAME := $(FLAMINGO_OTA_PACKAGE_NAME)-UNOFFICIAL
endif

ifeq ($(strip $(GAPPS_BUILD)),true)
FLAMINGO_OTA_PACKAGE_NAME := $(FLAMINGO_OTA_PACKAGE_NAME)-GAPPS
else
FLAMINGO_OTA_PACKAGE_NAME := $(FLAMINGO_OTA_PACKAGE_NAME)-VANILLA
endif
