// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStatsType, DisplayStatsSize } from './displayStats.enums';

export const BACKGROUND_CLASSES: Record<DisplayStatsType, string> = {
    [DisplayStatsType.Default]: 'display-stats-bg-default',
    [DisplayStatsType.Highlight]: 'display-stats-bg-highlight',
    [DisplayStatsType.Secondary]: 'display-stats-bg-secondary',
};

export const TEXT_CLASSES: Record<DisplayStatsType, string> = {
    [DisplayStatsType.Default]: 'display-stats-text-default',
    [DisplayStatsType.Highlight]: 'display-stats-text-highlight',
    [DisplayStatsType.Secondary]: 'display-stats-text-secondary',
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
