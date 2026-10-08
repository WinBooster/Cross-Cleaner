#!/bin/sh
# dpkg postinst for Cross Cleaner.
#
# nfpm takes a *path* here, not inline text: `scripts.postinstall` is read from
# disk and copied into the package as DEBIAN/postinst with mode 0755. Passing the
# script body inline instead fails with "file name too long", because nfpm tries
# to open the whole body as a filename.
#
# Runs under /bin/sh with a minimal PATH. Every command is guarded, so this is
# safe on a minimal container as well as on a desktop session.
set -e

# The hicolor icons installed by this package are not in any existing cache.
# Icon caches are per-user and dpkg cannot reach them, so the session bus is
# nudged instead when one exists. Absent over SSH and inside containers, hence
# the guard; the `|| true` keeps a read-only or missing cache from failing an
# otherwise good install.
if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qtf /usr/share/icons/hicolor || true
    gtk-update-icon-cache -qtf /usr/share/pixmaps || true
fi

# No ldconfig trigger: the package ships no shared library of its own.
# No user/group creation: it installs two files under /usr/bin.

exit 0
