// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SegmentedButtonType } from './segmented-button.enums';

export const BACKGROUND_COLORS: Record<SegmentedButtonType, string> = {
    [SegmentedButtonType.Outlined]: 'bg-transparent',
    [SegmentedButtonType.Filled]: 'bg-neutral-96 dark:bg-neutral-10',
};

export const OUTLINED_BORDER = 'border border-neutral-70 dark:border-neutral-40';
