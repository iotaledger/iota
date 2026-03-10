// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export { IotaNamesClient } from './iota-names-client.js';
export { IotaNamesTransaction } from './iota-names-transaction.js';
export type { IotaNamesClientConfig, NameRecord, Coupon } from './types.js';
export { ALLOWED_METADATA, MIN_LABEL_SIZE, GRACE_PERIOD_MS, packages } from './constants.js';
export {
    isSubname,
    isNestedSubname,
    validateYears,
    getConfigType,
    getNameType,
    getPricelistConfigType,
    getRenewalPricelistConfigType,
    getNameRegistrationType,
    getSubnameRegistrationType,
    getDenyListType,
} from './helpers.js';
export {
    isValidIotaName,
    validateIotaName,
    normalizeIotaName,
    validateIotaSubname,
} from './utils.js';
