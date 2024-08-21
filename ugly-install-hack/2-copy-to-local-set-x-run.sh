#!/usr/bin/env sh

SHARED_LOCATION_DIR=$(dirname "${0}")
rm -f /tmp/trombinoscope
cp "${SHARED_LOCATION_DIR}"/trombinoscope /tmp/trombinoscope
chmod 755 /tmp/trombinoscope
/tmp/trombinoscope "$@"
