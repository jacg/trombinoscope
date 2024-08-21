#!/usr/bin/env sh

TROMBI_TOP_DIR="$(dirname "$(dirname "${0}")")"
SHARED_INSTALL_DIR=${1}


echo TROMBI_TOP_DIR: "${TROMBI_TOP_DIR}"
echo SHARED_INSTALL_DIR: "${SHARED_INSTALL_DIR}"

echo cd "${TROMBI_TOP_DIR}"
cd "${TROMBI_TOP_DIR}"

cargo build --release
mkdir -p                                                      "${SHARED_INSTALL_DIR}"
cp "${TROMBI_TOP_DIR}"/target/release/trombinoscope           "${SHARED_INSTALL_DIR}"/trombinoscope
cp "${TROMBI_TOP_DIR}"/ugly-install-hack/2-copy-to-local-set-x-run.sh "${SHARED_INSTALL_DIR}"/trombinoscope.sh

echo ls -lh "${SHARED_INSTALL_DIR}"
ls -lh "${SHARED_INSTALL_DIR}"
