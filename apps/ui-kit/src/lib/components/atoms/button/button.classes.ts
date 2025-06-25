// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonSize, ButtonType } from './button.enums';

export const PADDINGS: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'px-md py-xs',
    [ButtonSize.Medium]: 'px-md py-sm',
};

export const PADDINGS_ONLY_ICON: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'p-xs',
    [ButtonSize.Medium]: 'p-sm',
};

export const BACKGROUND_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'button-bg-color-primary',
    [ButtonType.Secondary]: 'button-bg-color-secondary',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent button-border-color-outline',
    [ButtonType.Destructive]: 'button-bg-color-error',
};

export const DISABLED_BACKGROUND_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'button-bg-color-disabled-primary',
    [ButtonType.Secondary]: 'button-bg-color-secondary',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent button-border-color-outline',
    [ButtonType.Destructive]: 'button-bg-color-error',
};

const DEFAULT_TEXT_COLORS: string = 'button-text-color-neutral';

export const TEXT_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'button-text-color-primary',
    [ButtonType.Secondary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Ghost]: DEFAULT_TEXT_COLORS,
    [ButtonType.Outlined]: DEFAULT_TEXT_COLORS,
    [ButtonType.Destructive]: 'button-text-color-error',
};

export const TEXT_CLASSES: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'text-label-md',
    [ButtonSize.Medium]: 'text-label-lg',
};

export const TEXT_COLOR_DISABLED: Record<ButtonType, string> = {
    [ButtonType.Primary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Secondary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Ghost]: DEFAULT_TEXT_COLORS,
    [ButtonType.Outlined]: DEFAULT_TEXT_COLORS,
    [ButtonType.Destructive]: 'button-text-color-error',
};
