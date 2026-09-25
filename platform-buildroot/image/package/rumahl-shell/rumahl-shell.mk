################################################################################
# Prebuilt, target-matching rumahl platform and frontend deployment
################################################################################
RUMAHL_SHELL_VERSION = 0.1.0
RUMAHL_SHELL_SITE = $(call qstrip,$(BR2_PACKAGE_RUMAHL_SHELL_ARTIFACTS))
RUMAHL_SHELL_SITE_METHOD = local
RUMAHL_SHELL_LICENSE = Apache-2.0
RUMAHL_SHELL_LICENSE_FILES = usr/share/licenses/rumahl/LICENSE
RUMAHL_SHELL_DEPENDENCIES = nodejs nginx

define RUMAHL_SHELL_USERS
    rumahl-platform -1 rumahl-web -1 * /var/lib/rumahl /sbin/nologin rumahl-ssr rumahl-platform
    rumahl-shell -1 rumahl-ssr -1 * /nonexistent /sbin/nologin - rumahl-shell
    www-data 33 www-data 33 * - /sbin/nologin rumahl-web nginx
endef

define RUMAHL_SHELL_INSTALL_TARGET_CMDS
    test -x $(@D)/usr/bin/rumahl-platform-service
    cp -a $(@D)/usr/. $(TARGET_DIR)/usr/
    cp -a $(@D)/etc/. $(TARGET_DIR)/etc/
    $(INSTALL) -D -m 0644 $(BR2_EXTERNAL_RUMAHL_PATH)/nginx.conf $(TARGET_DIR)/etc/nginx/nginx.conf
endef

$(eval $(generic-package))
