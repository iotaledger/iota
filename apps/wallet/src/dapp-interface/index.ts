// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { registerWallet } from '@iota/wallet-standard';

import { IotaWallet } from './walletStandardInterface';

registerWallet(new IotaWallet());
