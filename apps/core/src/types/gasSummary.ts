// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GasCostSummary, IotaGasData } from '@iota/iota-sdk/client';

type Optional<T> = {
    [K in keyof T]?: T[K];
};

export type GasSummaryType =
    | (GasCostSummary &
          Optional<IotaGasData> & {
              totalGas?: string;
              owner?: string;
              isSponsored: boolean;
              gasUsed: GasCostSummary;
          })
    | null;
