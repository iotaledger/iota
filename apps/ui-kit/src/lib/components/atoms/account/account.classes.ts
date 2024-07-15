// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType } from './account.enums';

export const BACKGROUND_BADGE_COLORS = {
    [AccountType.Main]: 'bg-tertiary-90 dark:bg-tertiary-10',
    [AccountType.Legacy]: 'bg-error-90 dark:bg-tertiary-10',
};

export const TEXT_COLORS = {
    [AccountType.Main]: 'text-tertiary-20 dark:text-tertiary-90',
    [AccountType.Legacy]: 'text-error-10 dark:text-error-90',
};

export const BADGE_TEXT_CLASS = 'text-label-sm';
