// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SnackbarType } from './snackbar.enums';

export const TEXT_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'text-neutral-10 dark:text-neutral-92',
    [SnackbarType.Error]: 'text-error-20 dark:text-error-90',
    [SnackbarType.Warning]: 'text-warning-10 dark:text-warning-90',
    [SnackbarType.Success]: 'text-tertiary-20 dark:text-tertiary-90',
};

export const BACKGROUND_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'bg-primary-90 dark:bg-primary-10',
    [SnackbarType.Error]: 'bg-error-90 dark:bg-error-10',
    [SnackbarType.Warning]: 'bg-warning-90 dark:bg-warning-20',
    [SnackbarType.Success]: 'bg-tertiary-90 dark:bg-tertiary-10',
};
