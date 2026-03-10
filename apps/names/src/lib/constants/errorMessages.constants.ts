// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const GAS_BALANCE_TOO_LOW_ID = 'GasBalanceTooLow';
export const NOT_ENOUGH_BALANCE_ID = 'No valid gas coins found';
export const INSUFFICIENT_COIN_BALANCE_ID = 'InsufficientCoinBalance';
export const INVALID_IOTA_NAME = 'Invalid IOTA names';
export const INVALID_IOTA_OBJECT_ID = 'Invalid IOTA Object id';
export const INVALID_U64_VALUE = 'Invalid u64 value';
export const BLOCKED_OR_RESERVED_FUNCTION_NAME = 'assert_not_blocked_or_reserved';

export const NO_BALANCE_GENERIC_MESSAGE = 'Make sure you have enough funds and try again.';

export const CANT_RENEW_NAME_FOR_MORE_TIME = `Can't renew this name for more time.`;

// Default fallback message for unexpected errors
export const DEFAULT_ERROR_MESSAGE = 'Something went wrong. Please try again.';

// Common error patterns that should be caught and made user-friendly
export const ERROR_PATTERNS = {
    rejected: 'Transaction rejected by user.',
    'blind signing': 'Enable Blind Signing in the IOTA app settings on your Ledger device.',
    'fetch failed': 'Connection failed. Please check your internet connection and try again.',
    'network error': 'Network error. Please try again later.',
    'signal timed out': 'Request timed out. Please try again.',
    'failed to fetch': 'Unable to connect. Please check your connection and try again.',
    'transaction failed': 'Transaction failed. Please try again.',
    'insufficient funds': 'Insufficient funds to complete this transaction.',
    'gas limit exceeded': 'Transaction requires too much gas. Please try again.',
    unauthorized: "You don't have permission to perform this action.",
    'access denied': "Access denied. You don't have the required permissions.",
    forbidden: 'This action is not allowed.',
    FAILED_TO_COPY: 'Failed to copy to clipboard. Please try again.',
    [INVALID_IOTA_NAME]: 'Invalid name format. Please check your input.',
    [INVALID_U64_VALUE]: 'Invalid u64 value. Please check your input.',
    [INVALID_IOTA_OBJECT_ID]: 'Invalid object ID format. Please check your input.',
    [GAS_BALANCE_TOO_LOW_ID]: 'Not enough balance to cover transaction fees.',
    [NOT_ENOUGH_BALANCE_ID]: 'Not enough balance to create the transaction.',
    [INSUFFICIENT_COIN_BALANCE_ID]: 'Insufficient coin balance to complete this action.',
    'not valid for the given number of years': 'Coupon is not valid for the given number of years.',
    'not valid for the given name length': 'Coupon is not valid for the given name length.',
    'claimed the maximum number of times': 'Coupon has been claimed the maximum number of times.',
    'invalid percentage amount': 'Invalid percentage amount for coupon.',
    'address does not match': 'Coupon address does not match.',
    'coupon has expired': 'Coupon has expired.',
    'number of claims cannot be zero': 'Number of claims cannot be zero.',
    'cannot be used with other coupons': 'Coupon cannot be used with other coupons.',
    [BLOCKED_OR_RESERVED_FUNCTION_NAME]: 'This name is blocked or reserved and cannot be used.',
};
