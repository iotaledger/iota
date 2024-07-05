// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ThemedOrDefault } from '@/lib/types';
import { ButtonSize, ButtonType } from './button.enums';
import { Theme } from '@/lib/enums';

export const PADDINGS: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'px-md py-xs',
    [ButtonSize.Medium]: 'px-md py-sm',
};

export const PADDINGS_ONLY_ICON: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'p-xs',
    [ButtonSize.Medium]: 'p-sm',
};

export const BACKGROUND_COLORS: Record<ButtonType, ThemedOrDefault> = {
    [ButtonType.Primary]: 'bg-primary-30',
    [ButtonType.Secondary]: {
        [Theme.Light]: 'bg-neutral-90',
        [Theme.Dark]: 'bg-neutral-20',
    },
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90',
};

export const DISABLED_BACKGROUND_COLORS: Record<ButtonType, ThemedOrDefault> = {
    [ButtonType.Primary]: {
        [Theme.Light]: 'bg-neutral-80',
        [Theme.Dark]: 'bg-neutral-30',
    },
    [ButtonType.Secondary]: {
        [Theme.Light]: 'bg-neutral-90',
        [Theme.Dark]: 'bg-neutral-20',
    },
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90',
};

const DEFAULT_TEXT_COLORS: ThemedOrDefault = {
    [Theme.Light]: 'text-neutral-10',
    [Theme.Dark]: 'text-neutral-92',
};

export const TEXT_COLORS: Record<ButtonType, ThemedOrDefault> = {
    [ButtonType.Primary]: 'text-primary-100',
    [ButtonType.Secondary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Ghost]: DEFAULT_TEXT_COLORS,
    [ButtonType.Outlined]: DEFAULT_TEXT_COLORS,
    [ButtonType.Destructive]: 'text-error-20',
};

export const TEXT_CLASSES: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'text-label-md',
    [ButtonSize.Medium]: 'text-label-lg',
};

export const TEXT_COLOR_DISABLED: Record<ButtonType, ThemedOrDefault> = {
    [ButtonType.Primary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Secondary]: DEFAULT_TEXT_COLORS,
    [ButtonType.Ghost]: DEFAULT_TEXT_COLORS,
    [ButtonType.Outlined]: DEFAULT_TEXT_COLORS,
    [ButtonType.Destructive]: 'text-error-20',
};
