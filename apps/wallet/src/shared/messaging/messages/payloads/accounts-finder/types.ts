// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { MakeDerivationOptions } from '_src/background/account-sources/bip44Path';

export type SourceStrategyToFind =
    | {
          type: 'software';
          sourceID: string;
      }
    | {
          type: 'ledger';
          password: string;
      };

export type SourceStrategyToPersist =
    | {
          type: 'software';
          sourceID: string;
          bipPaths: MakeDerivationOptions[];
      }
    | {
          type: 'ledger';
          password: string;
          addresses: {
              address: string;
              derivationPath: string;
              publicKey: string;
          }[];
      };
