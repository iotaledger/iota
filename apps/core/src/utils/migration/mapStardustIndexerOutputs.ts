// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    STARDUST_STORAGE_DEPOSIT_RETURN_UC_TYPE,
    STARDUST_TIMELOCK_TYPE,
} from '../../constants';
import { StardustIndexerOutput } from './types';

type MapStardustOutput = (output: StardustIndexerOutput, type: string) => IotaObjectData;

export function mapStardustBasicOutputs(output: StardustIndexerOutput) {
    return mapStardustOutput(output, STARDUST_BASIC_OUTPUT_TYPE);
}

export function mapStardustNftOutputs(output: StardustIndexerOutput): IotaObjectData {
    return mapStardustOutput(output, STARDUST_NFT_OUTPUT_TYPE);
}

const mapStardustOutput: MapStardustOutput = function (
    output: StardustIndexerOutput,
    type: string,
) {
    return {
        objectId: output.id,
        digest: '',
        version: '',
        type: type,
        content: {
            dataType: 'moveObject' as const,
            type: type,
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
                metadata: output.metadata,
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
                          type: STARDUST_STORAGE_DEPOSIT_RETURN_UC_TYPE,
                          fields: {
                              return_address: output.storage_deposit_return.return_address,
                              return_amount: output.storage_deposit_return.return_amount,
                          },
                      }
                    : null,
                tag: output.tag,
                timelock_uc: output.timelock
                    ? {
                          fields: {
                              unix_time: output.timelock.unix_time,
                          },
                          type: STARDUST_TIMELOCK_TYPE,
                      }
                    : null,
            },
        },
    };
};
