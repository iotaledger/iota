// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE,
    STARDUST_PACKAGE_ID,
} from '../../constants';
import { StardustIndexerBasicOutput, StardustIndexerNftOutput } from './types';

export function mapStardustBasicOutputs(output: StardustIndexerBasicOutput) {
    return {
        objectId: output.id,
        digest: '',
        version: '',
        type: STARDUST_BASIC_OUTPUT_TYPE,
        content: {
            dataType: 'moveObject' as const,
            type: STARDUST_BASIC_OUTPUT_TYPE,
            fields: {
                balance: output.balance.value,
                expiration_uc: output.expiration
                    ? {
                          type: STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE,
                          fields: {
                              owner: output.expiration.owner,
                              return_address: output.expiration.return_address,
                              unix_time: output.expiration.unix_time,
                          },
                      }
                    : null,
                id: {
                    id: output.id,
                },
                metadata: [],
                native_tokens: {
                    type: '0x2::bag::Bag',
                    fields: {
                        id: {
                            id: output.native_tokens.id,
                        },
                        size: output.native_tokens.size,
                    },
                },
                sender: output.sender,
                storage_deposit_return_uc: output.storage_deposit_return
                    ? {
                          type: `${STARDUST_PACKAGE_ID}::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition`,
                          fields: {
                              return_address: output.storage_deposit_return.return_address,
                              return_amount: output.storage_deposit_return.return_address,
                          },
                      }
                    : null,
                tag: output.tag,
                timelock_uc: output.timelock
                    ? {
                          fields: {
                              unix_time: output.timelock.unix_time,
                          },
                          type: `${STARDUST_PACKAGE_ID}::timelock_unlock_condition::TimelockUnlockCondition`,
                      }
                    : null,
            },
        },
    };
}

export function mapStardustNftOutputs(output: StardustIndexerNftOutput) {
    return {
        objectId: output.id,
        digest: '',
        version: '',
        type: STARDUST_BASIC_OUTPUT_TYPE,
        content: {
            dataType: 'moveObject' as const,
            type: STARDUST_BASIC_OUTPUT_TYPE,
            fields: {
                balance: output.balance.value,
                expiration_uc: output.expiration
                    ? {
                          type: STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE,
                          fields: {
                              owner: output.expiration.owner,
                              return_address: output.expiration.return_address,
                              unix_time: output.expiration.unix_time,
                          },
                      }
                    : null,
                id: {
                    id: output.id,
                },
                metadata: [],
                native_tokens: {
                    type: '0x2::bag::Bag',
                    fields: {
                        id: {
                            id: output.native_tokens.id,
                        },
                        size: output.native_tokens.size,
                    },
                },
                sender: output.sender,
                storage_deposit_return_uc: output.storage_deposit_return
                    ? {
                          type: `${STARDUST_PACKAGE_ID}::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition`,
                          fields: {
                              return_address: output.storage_deposit_return.return_address,
                              return_amount: output.storage_deposit_return.return_address,
                          },
                      }
                    : null,
                tag: output.tag,
                timelock_uc: output.timelock
                    ? {
                          fields: {
                              unix_time: output.timelock.unix_time,
                          },
                          type: `${STARDUST_PACKAGE_ID}::timelock_unlock_condition::TimelockUnlockCondition`,
                      }
                    : null,
            },
        },
    };
}
