// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaGasCostSummary, IotaGasData } from '@iota/iota-sdk/client';

type Optional<T> = {
    [K in keyof T]?: T[K];
};

export type GasSummaryType =
    | (IotaGasCostSummary &
          Optional<IotaGasData> & {
              isSponsored: boolean;
              gasUsed: IotaGasCostSummary;
              totalGas?: string;
              owner?: string;
          })
    | null;
