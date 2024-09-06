// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStatsBackground, DisplayStatsSize } from './display-stats.enums';

export const BACKGROUND_CLASSES: Record<DisplayStatsBackground, string> = {
    [DisplayStatsBackground.Default]: 'bg-neutral-96 dark:bg-neutral-10',
    [DisplayStatsBackground.Highlight]: 'bg-primary-30 dark:bg-primary-80',
    [DisplayStatsBackground.Secondary]: 'bg-secondary-90 dark:bg-secondary-10',
};

export const TEXT_CLASSES: Record<DisplayStatsBackground, string> = {
    [DisplayStatsBackground.Default]: 'text-neutral-10 dark:text-neutral-60',
    [DisplayStatsBackground.Highlight]: 'text-neutral-100 dark:text-primary-10',
    [DisplayStatsBackground.Secondary]: 'text-neutral-10 dark:text-neutral-60',
};

export const SIZE_CLASSES: Record<DisplayStatsSize, string> = {
    [DisplayStatsSize.Default]: 'h-24',
    [DisplayStatsSize.Large]: 'h-40',
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
