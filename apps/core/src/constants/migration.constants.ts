// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export const STARDUST_PACKAGE_ID =
    '0xfc113cec199fec6b0be60cf8a55fd39208dd4c67a94b76698c093b096e9f338d';
export const STARDUST_BASIC_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::basic_output::BasicOutput<${IOTA_TYPE_ARG}>`;
export const STARDUST_NFT_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::nft_output::NftOutput<${IOTA_TYPE_ARG}>`;
