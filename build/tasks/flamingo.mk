# Copyright (C) 2007 The Android Open Source Project
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

FLAMINGO_OTA := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)

$(FLAMINGO_OTA): $(BUILT_TARGET_FILES_PACKAGE) $(OTA_FROM_TARGET_FILES)
	$(call build-ota-package-target,$@,-k $(FLAMINGO_KEY_CERT_PAIR) --output_metadata_path $(INTERNAL_OTA_METADATA))

.PHONY: flamingo
flamingo: $(FLAMINGO_OTA)
	$(hide) mv $(FLAMINGO_OTA) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M").zip
	@echo "Flamingo full OTA package is ready"

ifneq ($(strip $(PREVIOUS_TARGET_FILES_PACKAGE)),)
FLAMINGO_INCREMENTAL_OTA := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-incremental

$(FLAMINGO_INCREMENTAL_OTA): $(BUILT_TARGET_FILES_PACKAGE) $(OTA_FROM_TARGET_FILES)
	$(OTA_FROM_TARGET_FILES) \
	--block \
	-p $(SOONG_HOST_OUT) \
	-k $(DEFAULT_KEY_CERT_PAIR) \
	-i $(PREVIOUS_TARGET_FILES_PACKAGE) \
	$(BUILT_TARGET_FILES_PACKAGE) $@

.PHONY: flamingo-incremental
flamingo-incremental: $(FLAMINGO_INCREMENTAL_OTA)
	$(hide) mv $(FLAMINGO_INCREMENTAL_OTA) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-incremental-$(shell date "+%Y%m%d-%H%M").zip
	@echo "Flamingo incremental OTA package is ready"
endif

FLAMINGO_FASTBOOT_PACKAGE := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-img

$(FLAMINGO_FASTBOOT_PACKAGE): $(BUILT_TARGET_FILES_PACKAGE) $(IMG_FROM_TARGET_FILES)
	$(IMG_FROM_TARGET_FILES) \
	$(BUILT_TARGET_FILES_PACKAGE) $@

.PHONY: flamingo-fastboot
flamingo-fastboot: $(FLAMINGO_FASTBOOT_PACKAGE)
	$(hide) mv $(FLAMINGO_FASTBOOT_PACKAGE) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-img.zip
	@echo "Flamingo fastboot package is ready"

.PHONY: flamingo-boot
flamingo-boot: $(BUILT_TARGET_FILES_PACKAGE)
	$(hide) cp <(unzip -o -q -p $(BUILT_TARGET_FILES_PACKAGE) IMAGES/boot.img) \
	$(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-boot.img
	@echo "Boot image copied"