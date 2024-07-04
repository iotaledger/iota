// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonSize, ButtonType } from './button.enums';

export const PADDINGS: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'px-4 py-2',
    [ButtonSize.Medium]: 'px-4 py-3',
};

export const PADDINGS_ONLY_ICON: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'p-2',
    [ButtonSize.Medium]: 'p-3',
};

export const BACKGROUND_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'bg-primary-30',
    [ButtonType.Secondary]: 'bg-neutral-90',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90',
};

export const DISABLED_BACKGROUND_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'bg-neutral-80',
    [ButtonType.Secondary]: 'bg-neutral-90',
    [ButtonType.Ghost]: 'bg-transparent',
    [ButtonType.Outlined]: 'bg-transparent border border-neutral-50',
    [ButtonType.Destructive]: 'bg-error-90',
};

export const TEXT_COLORS: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'text-primary-100',
    [ButtonType.Secondary]: 'text-neutral-10',
    [ButtonType.Ghost]: 'text-neutral-10',
    [ButtonType.Outlined]: 'text-neutral-10',
    [ButtonType.Destructive]: 'text-error-20',
};

export const TEXT_CLASSES: Record<ButtonSize, string> = {
    [ButtonSize.Small]: 'text-label-md',
    [ButtonSize.Medium]: 'text-label-lg',
};

export const TEXT_COLOR_DISABLED: Record<ButtonType, string> = {
    [ButtonType.Primary]: 'text-neutral-10',
    [ButtonType.Secondary]: 'text-neutral-10',
    [ButtonType.Ghost]: 'text-neutral-10',
    [ButtonType.Outlined]: 'text-neutral-10',
    [ButtonType.Destructive]: 'text-error-20',
};
