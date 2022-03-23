TARGET_BOOT_ANIMATION_RES ?= undefined

ifeq ($(TARGET_BOOT_ANIMATION_RES),720)
     PRODUCT_COPY_FILES += vendor/prebuilts/bootanimation/720.zip:$(TARGET_COPY_OUT_SYSTEM)/media/bootanimation.zip
else ifeq ($(TARGET_BOOT_ANIMATION_RES),1080)
     PRODUCT_COPY_FILES += vendor/prebuilts/bootanimation/1080.zip:$(TARGET_COPY_OUT_SYSTEM)/media/bootanimation.zip
else ifeq ($(TARGET_BOOT_ANIMATION_RES),1440)
     PRODUCT_COPY_FILES += vendor/prebuilts/bootanimation/1440.zip:$(TARGET_COPY_OUT_SYSTEM)/media/bootanimation.zip
else
     $(warning Defined bootanimation res is wrong or you forgot to define it, using 1080p bootanimation )
     PRODUCT_COPY_FILES += vendor/prebuilts/bootanimation/1080.zip:$(TARGET_COPY_OUT_SYSTEM)/media/bootanimation.zip
endif
