// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChipType } from './chip.enums';

export const ROUNDED_CLASS = 'rounded-full';

export const BACKGROUND_CLASSES: Record<ChipType, string> = {
    [ChipType.Default]: 'bg-transparent',
    [ChipType.Selected]: 'chip-bg-selected',
    [ChipType.Success]: 'bg-success-surface',
    [ChipType.Brand]: 'chip-bg-brand',
    [ChipType.Error]: 'bg-error-surface',
};

const STATE_LAYER_OUTLINE =
    'outline outline-1 outline-transparent hover:outline-shader-primary-light-8 active:outline-shader-primary-light-12 dark:hover:outline-shader-primary-dark-8 dark:active:outline-shader-primary-dark-12';

const STATE_LAYER_BG_CLASSES =
    'hover:bg-shader-primary-light-8 active:bg-shader-primary-light-12 dark:hover:bg-shader-primary-dark-8 dark:active:bg-shader-primary-dark-12 focus:bg-shader-primary-light-12 dark:focus:bg-shader-primary-dark-12';

export const STATE_LAYER_CLASSES = `${STATE_LAYER_OUTLINE} ${STATE_LAYER_BG_CLASSES}`;

export const BORDER_CLASSES: Record<ChipType, string> = {
    [ChipType.Default]: 'chip-border-default',
    [ChipType.Selected]: 'border-transparent',
    [ChipType.Success]: 'border-success-surface',
    [ChipType.Brand]: 'chip-border-color-brand',
    [ChipType.Error]: 'border-error-surface',
};
export const TEXT_COLOR: Record<ChipType, string> = {
    [ChipType.Default]: 'chip-text-default',
    [ChipType.Selected]: 'chip-text-secondary',
    [ChipType.Success]: 'chip-text-secondary',
    [ChipType.Brand]: 'chip-text-brand',
    [ChipType.Error]: 'chip-text-secondary',
};

export const FOCUS_CLASSES =
    'focus-visible:shadow-[0_0_0_2px] focus-visible:chip-focus-ring focus-visible:outline-none';
