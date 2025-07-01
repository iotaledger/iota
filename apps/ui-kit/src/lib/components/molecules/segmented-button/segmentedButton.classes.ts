// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SegmentedButtonType } from './segmentedButton.enums';

export const BACKGROUND_COLORS: Record<SegmentedButtonType, string> = {
    [SegmentedButtonType.Outlined]: 'bg-transparent',
    [SegmentedButtonType.Filled]: 'segmented-filled-bg-color',
    [SegmentedButtonType.Transparent]: 'bg-transparent',
};

export const OUTLINED_BORDER = 'border segmented-outlined-border';
