// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DividerType } from './divider.enums';

export const BACKGROUND_COLORS = 'bg-shader-neutral-light-8 dark:bg-shader-neutral-dark-8';

export const DIVIDER_FULL_WIDTH: Record<DividerType, string> = {
    [DividerType.Horizontal]: 'w-full',
    [DividerType.Vertical]: 'h-full',
};
