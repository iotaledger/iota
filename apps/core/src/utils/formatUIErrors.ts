// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const DRY_RUN_UI_ERROR_TITLE = 'Transaction failed during dry run';
const DRY_RUN_ERROR_DESCRIPTIONS: { match: string; description: string }[] = [
    {
        match: 'InsufficientGas',
        description: 'Not enough gas to complete this transaction.',
    },
    {
        match: 'InvalidGasObject',
        description: 'The selected gas object is invalid or unavailable.',
    },
    {
        match: 'FeatureNotYetSupported',
        description: 'This feature is not supported.',
    },
    {
        match: 'MoveObjectTooBig',
        description: 'One of the objects is too large for this transaction.',
    },
    {
        match: 'InsufficientCoinBalance',
        description: 'Not enough balance to cover the amount.',
    },
    {
        match: 'CoinBalanceOverflow',
        description: 'Coin balance overflow while processing the amounts.',
    },
    {
        match: 'FunctionNotFound',
        description: 'The target function was not found in the package.',
    },
    {
        match: 'CommandArgumentError',
        description: 'One or more transaction arguments are invalid.',
    },
    {
        match: 'TypeArgumentError',
        description: 'One or more type arguments are invalid.',
    },
    {
        match: 'InputObjectDeleted',
        description: 'An input object was deleted or no longer exists.',
    },
];

export function getUserFriendlyDryRunExecutionError(errorText: string): string {
    const matched = DRY_RUN_ERROR_DESCRIPTIONS.find(({ match }) => errorText.includes(match));
    return matched ? matched.description : errorText;
}
