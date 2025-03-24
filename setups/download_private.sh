#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

if ! cosign version &> /dev/null
then
    echo "cosign in not installed, Please install cosign for binary verification."
    echo "https://docs.sigstore.dev/cosign/installation"
    exit
fi

commit_sha=$1
pub_key=https://iota-private.s3.us-west-2.amazonaws.com/iota_security_release.pem
url=https://iota-releases.s3-accelerate.amazonaws.com/$commit_sha

echo "[+] Downloading iota binaries for $commit_sha ..."
curl $url/iota -o iota
curl $url/iota-indexer -o iota-indexer
curl $url/iota-node -o iota-node
curl $url/iota-tool -o iota-tool

echo "[+] Verifying iota binaries for $commit_sha ..."
cosign verify-blob --insecure-ignore-tlog --key $pub_key --signature $url/iota.sig iota
cosign verify-blob --insecure-ignore-tlog --key $pub_key --signature $url/iota-indexer.sig iota-indexer
cosign verify-blob --insecure-ignore-tlog --key $pub_key --signature $url/iota-node.sig iota-node
cosign verify-blob --insecure-ignore-tlog --key $pub_key --signature $url/iota-tool.sig iota-tool
