// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBoxType } from './infoBox.enums';

export const ICON_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-primary-90 dark:bg-primary-10 text-primary-20 dark:text-primary-90',
    [InfoBoxType.Error]: 'bg-error-90 dark:bg-error-10 text-error-20 dark:text-error-90',
    [InfoBoxType.Success]:
        'bg-tertiary-90 dark:bg-tertiary-10 text-tertiary-20 dark:text-tertiary-90',
    [InfoBoxType.Warning]: 'bg-warning-90 dark:bg-warning-20 text-warning-10 dark:text-warning-90',
};

export const BACKGROUND_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-primary-90 dark:bg-primary-10',
    [InfoBoxType.Error]: 'bg-error-90 dark:bg-error-10',
    [InfoBoxType.Success]: 'bg-tertiary-90 dark:bg-tertiary-10',
    [InfoBoxType.Warning]: 'bg-warning-90 dark:bg-warning-20',
};
