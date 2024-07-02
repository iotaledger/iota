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
binary_name=$2
pub_key=https://iota-private.s3.us-west-2.amazonaws.com/iota_security_release.pem
url=https://iota-releases.s3-accelerate.amazonaws.com/$commit_sha

echo "[+] Downloading binary '$binary_name' for $commit_sha ..."
curl $url/$binary_name -o $binary_name

echo "[+] Verifying binary '$binary_name' for $commit_sha ..."
cosign verify-blob --insecure-ignore-tlog --key $pub_key --signature $url/$binary_name.sig $binary_name
