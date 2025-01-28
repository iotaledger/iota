// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export const STARDUST_PACKAGE_ID =
    '0x000000000000000000000000000000000000000000000000000000000000107a';
export const STARDUST_BASIC_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::basic_output::BasicOutput<${IOTA_TYPE_ARG}>`;
export const STARDUST_NFT_OUTPUT_TYPE = `${STARDUST_PACKAGE_ID}::nft_output::NftOutput<${IOTA_TYPE_ARG}>`;
export const STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE = `${STARDUST_PACKAGE_ID}::expiration_unlock_condition::ExpirationUnlockCondition`;
export const STARDUST_STORAGE_DEPOSIT_RETURN_UC_TYPE = `${STARDUST_PACKAGE_ID}::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition`;
export const STARDUST_TIMELOCK_TYPE = `${STARDUST_PACKAGE_ID}::timelock_unlock_condition::TimelockUnlockCondition`;
