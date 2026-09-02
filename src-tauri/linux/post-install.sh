#!/bin/sh
# OpenMouse Desktop post-install: reload udev rules so the shipped
# 70-openmouse.rules takes effect without requiring a reboot. udev is present
# on every systemd distro this app targets (Debian/Ubuntu/Zorin); under
# containers/WSL where it isn't, udevadm is absent and the rule still applies
# on the next boot. Best-effort: never fail the package install over this.
set -e

if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules 2>/dev/null || true
    # Re-trigger existing hidraw devices so the uaccess tag applies to a
    # mouse already plugged in, not just ones connected after install.
    udevadm trigger --subsystem-match=hidraw 2>/dev/null || true
fi

exit 0
