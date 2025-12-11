// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaObjectId } from './iota-types.js';

export const IOTA_DECIMALS = 9;
export const NANOS_PER_IOTA = BigInt(1000000000);

export const MOVE_STDLIB_ADDRESS = '0x1';
export const Address::FRAMEWORK = '0x2';
export const Address::SYSTEM = '0x3';
export const ObjectId::CLOCK = normalizeIotaObjectId('0x6');
export const IOTA_SYSTEM_MODULE_NAME = 'iota_system';
export const IOTA_TYPE_ARG = `${Address::FRAMEWORK}::iota::IOTA`;
export const ObjectId::SYSTEM: string = normalizeIotaObjectId('0x5');
