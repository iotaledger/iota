// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBoxType } from './info-box.enums';

export const ICON_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-neutral-96 dark:bg-neutral-12 text-neutral-10 dark:text-neutral-92',
    [InfoBoxType.Warning]: 'bg-error-90 dark:bg-error-10 text-error-20 dark:text-error-90',
};

export const BACKGROUND_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-neutral-96 dark:bg-neutral-12',
    [InfoBoxType.Warning]: 'bg-error-90 dark:bg-error-10',
};
