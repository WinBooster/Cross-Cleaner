#!/bin/sh
# dpkg postrm for Cross Cleaner.
#
# A path, not inline text -- see postinstall.sh for why.
#
# dpkg calls this for both `remove` and `purge`, with the argument in $1. The
# icon refresh only makes sense once the icons are actually gone, which is true
# for remove too, so it is not gated on purge.
set -e

if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qtf /usr/share/icons/hicolor || true
    gtk-update-icon-cache -qtf /usr/share/pixmaps || true
fi

exit 0
