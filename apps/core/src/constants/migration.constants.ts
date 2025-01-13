// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export const STARDUST_PACKAGE_ID =
    '000000000000000000000000000000000000000000000000000000000000107a';
export const STARDUST_BASIC_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::basic_output::BasicOutput<${IOTA_TYPE_ARG}>`;
export const STARDUST_NFT_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::nft_output::NftOutput<${IOTA_TYPE_ARG}>`;
