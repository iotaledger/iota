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
    [ButtonType.Primary]: 'bg-primary-30',
    [ButtonType.Secondary]: 'bg-neutral-90 dark:bg-neutral-20',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90 dark:bg-error-20',
};

export const DISABLED_BACKGROUND_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'bg-neutral-80 dark:bg-neutral-30',
    [ButtonType.Secondary]: 'bg-neutral-90 dark:bg-neutral-20',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90 dark:bg-error-20',
};

const DEFAULT_TEXT_COLORS: string = 'text-neutral-10 dark:text-neutral-92';

export const TEXT_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'text-primary-100',
    [ButtonType.Secondary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Ghost]: DEFAULT_TEXT_COLORS,
    [ButtonType.Outlined]: DEFAULT_TEXT_COLORS,
    [ButtonType.Destructive]: 'text-error-20 dark:text-error-90',
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
    [ButtonType.Destructive]: 'text-error-20 dark:text-error-90',
};
