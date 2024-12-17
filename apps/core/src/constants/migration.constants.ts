// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export const STARDUST_PACKAGE_ID =
    '0x22c2abf47e7033c79be1ee80f53bb5b093bea2ca8a677689aadeb82ae4d53a65';
export const STARDUST_BASIC_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::basic_output::BasicOutput<${IOTA_TYPE_ARG}>`;
export const STARDUST_NFT_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::nft_output::NftOutput<${IOTA_TYPE_ARG}>`;
