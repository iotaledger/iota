// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DividerType } from './divider.enums';

// todo change the colors to the actual colors Light=> #002F6D, Dark=> #BED8FF
export const BACKGROUND_COLORS = ['bg-neutral-90', 'dark:bg-neutral-20'];

export const DIVIDER_FULL_WIDTH: Record<DividerType, string> = {
    [DividerType.Horizontal]: 'w-full',
    [DividerType.Vertical]: 'h-full',
};
