// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStatsType, DisplayStatsSize } from './displayStats.enums';

export const BACKGROUND_CLASSES: Record<DisplayStatsType, string> = {
    [DisplayStatsType.Default]: 'bg-neutral-96 dark:bg-neutral-10',
    [DisplayStatsType.Highlight]: 'bg-primary-30 dark:bg-primary-80',
    [DisplayStatsType.Secondary]: 'bg-secondary-90 dark:bg-secondary-10',
};

export const TEXT_CLASSES: Record<DisplayStatsType, string> = {
    [DisplayStatsType.Default]: 'text-neutral-10 dark:text-neutral-92',
    [DisplayStatsType.Highlight]: 'text-neutral-100 dark:text-primary-10',
    [DisplayStatsType.Secondary]: 'text-neutral-10 dark:text-neutral-92',
};

export const SIZE_CLASSES: Record<DisplayStatsSize, string> = {
    [DisplayStatsSize.Default]: 'gap-y-sm',
    [DisplayStatsSize.Large]: 'gap-y-md',
};

export const VALUE_TEXT_CLASSES: Record<DisplayStatsSize, string> = {
    [DisplayStatsSize.Default]: 'text-title-md',
    [DisplayStatsSize.Large]: 'text-headline-sm',
};

export const SUPPORTING_LABEL_TEXT_CLASSES: Record<DisplayStatsSize, string> = {
    [DisplayStatsSize.Default]: 'text-label-md',
    [DisplayStatsSize.Large]: 'text-label-lg',
};

export const LABEL_TEXT_CLASSES: Record<DisplayStatsSize, string> = {
    [DisplayStatsSize.Default]: 'text-label-sm',
    [DisplayStatsSize.Large]: 'text-label-md',
};
