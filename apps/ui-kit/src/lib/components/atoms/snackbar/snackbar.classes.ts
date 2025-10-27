// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SnackbarType } from './snackbar.enums';

export const TEXT_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'text-on-default',
    [SnackbarType.Error]: 'text-on-error',
    [SnackbarType.Warning]: 'text-on-warning',
    [SnackbarType.Success]: 'text-on-success',
};

export const BACKGROUND_COLOR: Record<SnackbarType, string> = {
    [SnackbarType.Default]: 'bg-default-surface',
    [SnackbarType.Error]: 'bg-error-surface',
    [SnackbarType.Warning]: 'bg-warning-surface',
    [SnackbarType.Success]: 'bg-success-surface',
};
