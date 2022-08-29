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

FLAMINGO_OTA := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME).zip

intermediates_dir := $(call intermediates-dir-for,PACKAGING,target_files)
GENERATED_TARGET_FILES_PACKAGE := $(intermediates_dir)/generated-$(TARGET_PRODUCT)-target_files-$(FILE_NAME_TAG).zip

CERTS_DIR ?= certs
ifneq ($(wildcard $(CERTS_DIR)/releasekey.*),)
SIGNING_KEY := $(CERTS_DIR)/releasekey
else

ifeq ($(OFFICIAL_BUILD),true)
$(error "Official builds must be signed with release keys, run keygen to generate signing keys")
endif

endif

SIGN_TARGET_FILES_APKS := $(HOST_OUT_EXECUTABLES)/sign_target_files_apks$(HOST_EXECUTABLE_SUFFIX)
$(GENERATED_TARGET_FILES_PACKAGE): $(BUILT_TARGET_FILES_PACKAGE) $(SIGN_TARGET_FILES_APKS)
	if [ -n "$(SIGNING_KEY)" ] ; then \
		$(SIGN_TARGET_FILES_APKS) -o \
			--default_key_mappings $(CERTS_DIR) \
			$(BUILT_TARGET_FILES_PACKAGE) $@; \
		rm -rf $(BUILT_TARGET_FILES_PACKAGE); \
	else \
		mv $(BUILT_TARGET_FILES_PACKAGE) $@; \
	fi

SIGNING_KEY ?= build/make/target/product/security/testkey

$(FLAMINGO_OTA): $(GENERATED_TARGET_FILES_PACKAGE) $(OTA_FROM_TARGET_FILES)
	$(OTA_FROM_TARGET_FILES) \
		--block \
		-p $(SOONG_HOST_OUT) \
		-k $(SIGNING_KEY) \
		$(GENERATED_TARGET_FILES_PACKAGE) $@

.PHONY: flamingo
flamingo: $(FLAMINGO_OTA)
	$(hide) mv $(FLAMINGO_OTA) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-full.zip
	@echo "Flamingo full OTA package is ready"

ifneq ($(strip $(PREVIOUS_TARGET_FILES_PACKAGE)),)
FLAMINGO_INCREMENTAL_OTA := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-incremental.zip

$(FLAMINGO_INCREMENTAL_OTA): $(GENERATED_TARGET_FILES_PACKAGE) $(OTA_FROM_TARGET_FILES)
	$(OTA_FROM_TARGET_FILES) \
	--block \
	-p $(SOONG_HOST_OUT) \
	-k $(SIGNING_KEY) \
	-i $(PREVIOUS_TARGET_FILES_PACKAGE) \
	$(GENERATED_TARGET_FILES_PACKAGE) $@

.PHONY: flamingo-incremental
flamingo-incremental: $(FLAMINGO_INCREMENTAL_OTA)
	$(hide) mv $(FLAMINGO_INCREMENTAL_OTA) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-incremental.zip
	@echo "Flamingo incremental OTA package is ready"
endif

FLAMINGO_FASTBOOT_PACKAGE := $(PRODUCT_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-fastboot.zip

$(FLAMINGO_FASTBOOT_PACKAGE): $(GENERATED_TARGET_FILES_PACKAGE) $(IMG_FROM_TARGET_FILES)
	$(IMG_FROM_TARGET_FILES) \
	$(GENERATED_TARGET_FILES_PACKAGE) $@

.PHONY: flamingo-fastboot
flamingo-fastboot: $(FLAMINGO_FASTBOOT_PACKAGE)
	$(hide) mv $(FLAMINGO_FASTBOOT_PACKAGE) $(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-fastboot.zip
	@echo "Flamingo fastboot package is ready"

.PHONY: flamingo-boot
flamingo-boot: $(GENERATED_TARGET_FILES_PACKAGE)
	$(hide) cp <(unzip -o -q -p $(GENERATED_TARGET_FILES_PACKAGE) IMAGES/boot.img) \
		$(FLAMINGO_OUT)/$(FLAMINGO_OTA_PACKAGE_NAME)-$(shell date "+%Y%m%d-%H%M")-boot.img
	@echo "Boot image copied"