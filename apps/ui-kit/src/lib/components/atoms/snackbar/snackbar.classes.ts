// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SnackbarType } from './snackbar.enums';

export const TEXT_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'text-neutral-10 dark:text-neutral-92',
    [SnackbarType.Error]: 'text-error-20 dark:text-error-90',
};

export const BACKGROUND_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'bg-neutral-80 dark:bg-neutral-30',
    [SnackbarType.Error]: 'bg-error-90 dark:bg-error-10',
};
