// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { FiatTokenName } from '../enums';

export const COIN_TYPE_TO_FIAT_TOKEN_NAME: Record<string, FiatTokenName> = {
    [IOTA_TYPE_ARG]: FiatTokenName.IOTA,
};
