// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChipType } from './chip.enums';

export const ROUNDED_CLASS = 'rounded-full';

export const BACKGROUND_CLASSES: Record<ChipType, string> = {
    [ChipType.Outline]: 'bg-transparent',
    [ChipType.Elevated]: 'chip-bg-elevated',
    [ChipType.Success]: 'bg-success-surface',
    [ChipType.Brand]: 'chip-bg-brand',
    [ChipType.Error]: 'bg-error-surface',
};

export const STATE_LAYER_OUTLINE =
    'outline outline-1 hover:chip-outline-color-hover active:chip-outline-color-active';

export const STATE_LAYER_BG_CLASSES =
    'hover:chip-bg-color-hover active:chip-bg-color-active group-focus:chip-bg-color-focus';

export const STATE_LAYER_CLASSES = `${STATE_LAYER_OUTLINE} ${STATE_LAYER_BG_CLASSES}`;

export const BG_SELECTED_OUTLINE: Partial<Record<ChipType, string>> = {
    [ChipType.Outline]: 'chip-bg-selected-outline',
};

export const BG_SELECTED_OVERLAY = 'chip-bg-selected-overlay';

export const TEXT_COLOR_SELECTED_OUTLINE: Partial<Record<ChipType, string>> = {
    [ChipType.Outline]: 'chip-text-secondary',
};

export const BORDER_CLASSES: Record<ChipType, string> = {
    [ChipType.Outline]: 'chip-border-default',
    [ChipType.Elevated]: 'border-transparent',
    [ChipType.Success]: 'border-success-surface',
    [ChipType.Brand]: 'chip-border-color-brand',
    [ChipType.Error]: 'border-error-surface',
};

export const TEXT_COLOR: Record<ChipType, string> = {
    [ChipType.Outline]: 'chip-text-default',
    [ChipType.Elevated]: 'chip-text-secondary',
    [ChipType.Success]: 'chip-text-secondary',
    [ChipType.Brand]: 'chip-text-brand',
    [ChipType.Error]: 'chip-text-secondary',
};

export const FOCUS_CLASSES =
    'focus-visible:shadow-[0_0_0_2px] focus-visible:chip-focus-ring focus-visible:outline-none';

export const CLOSE_ICON_INTERACTIVE =
    'chip-close-icon-opacity group-hover:opacity-100 group-focus:opacity-100 group-active:opacity-100';
