// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type Stake =
    | {
          id: string;
          validator: string;
          stake: string;
          rewards: string;
          stakeActiveEpoch: string;
          stakeRequestEpoch: string;
          status: 'Pending';
      }
    | {
          id: string;
          validator: string;
          stake: string;
          rewards: string;
          stakeActiveEpoch: string;
          stakeRequestEpoch: string;
          estimatedReward: string;
          status: 'Active';
      }
    | {
          id: string;
          validator: string;
          stake: string;
          rewards: string;
          stakeActiveEpoch: string;
          stakeRequestEpoch: string;
          status: 'Unstaked';
      };
