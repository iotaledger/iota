// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Timelocked stake: fields.staked_iota.fields.{pool_id, stake_activation_epoch}
export interface TimelockedStakeObjectInput {
    objectId: string;
    content: {
        dataType: 'moveObject';
        fields: {
            staked_iota: {
                fields: {
                    pool_id: string;
                    stake_activation_epoch: string;
                };
            };
        };
    };
}

// Regular stake: fields.{pool_id, stake_activation_epoch}
export interface RegularStakeObjectInput {
    objectId: string;
    content: {
        dataType: 'moveObject';
        fields: {
            pool_id: string;
            stake_activation_epoch: string;
        };
    };
}
