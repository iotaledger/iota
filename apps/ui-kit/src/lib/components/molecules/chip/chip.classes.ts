// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChipState } from './chip.enums';

export const ROUNDED_CLASS = 'rounded-full';

export const BACKGROUND_CLASSES: Record<ChipState, string> = {
    [ChipState.Default]: 'bg-transparent',
    [ChipState.Selected]: 'bg-primary-90 dark:bg-primary-10',
};

const STATE_LAYER_OUTLINE =
    'outline outline-1 outline-transparent hover:outline-shader-primary-light-8 active:outline-shader-primary-light-12 dark:hover:outline-shader-primary-dark-8 dark:active:outline-shader-primary-dark-12';

const STATE_LAYER_BG_CLASSES =
    'hover:bg-shader-primary-light-8 active:bg-shader-primary-light-12 dark:hover:bg-shader-primary-dark-8 dark:active:bg-shader-primary-dark-12 focus:bg-shader-primary-light-12 dark:focus:bg-shader-primary-dark-12';

export const STATE_LAYER_CLASSES = `${STATE_LAYER_OUTLINE} ${STATE_LAYER_BG_CLASSES}`;

export const BORDER_CLASSES: Record<ChipState, string> = {
    [ChipState.Default]: 'border-neutral-70 dark:border-neutral-40',
    [ChipState.Selected]: 'border-transparent',
};

export const TEXT_COLOR: Record<ChipState, string> = {
    [ChipState.Default]: 'text-neutral-40 dark:text-neutral-60',
    [ChipState.Selected]: 'text-neutral-10 dark:text-neutral-92',
};

export const FOCUS_CLASSES =
    'focus-visible:shadow-[0_0_0_2px] focus-visible:shadow-primary-30 focus-visible:outline-none dark:focus-visible:shadow-primary-80';
