// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FiatTokenName } from '@iota/core/enums/fiatTokenName.enums';

export const tokenPriceKey = (coinName: string) => `tokenPrice${coinName}`;
export const TOKEN_PRICE_CURRENCY = 'usd';
export const TOKEN_PRICE_COINS = [FiatTokenName.IOTA];
