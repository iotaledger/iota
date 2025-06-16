// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ToggleSize } from './toggle.enums';

export const TOGGLE = 'relative inline-flex items-center p-xxs border rounded-full cursor-pointer';

export const TOGGLE_STATES = {
    active: 'bg-primary-30 border-primary-30',
    inactive: 'bg-primary-100 dark:bg-neutral-6 border-neutral-70 dark:border-neutral-40',
    disabledActive: 'bg-neutral-70 border-neutral-70',
    disabled: 'opacity-40 cursor-not-allowed',
};

export const TOGGLE_SIZE = {
    [ToggleSize.Small]: 'h-5 w-10',
    [ToggleSize.Default]: 'h-6 w-12',
};

export const TOGGLE_THUMB = 'absolute rounded-full transition-all duration-200 ease-in';
export const TOGGLE_THUMB_POSITION = {
    [ToggleSize.Small]: {
        unchecked: 'left-1',
        checked: 'left-8 -translate-x-full',
    },
    [ToggleSize.Default]: {
        unchecked: 'left-1',
        checked: 'left-10 -translate-x-full',
    },
};
export const TOGGLE_THUMB_COLOR = {
    unchecked: 'bg-neutral-60',
    checked: 'bg-white',
};
export const TOGGLE_THUMB_SIZE = {
    [ToggleSize.Small]: 'h-3 w-3',
    [ToggleSize.Default]: 'h-4 w-4',
};

export const TOGGLE_CONTAINER = 'inline-flex items-center gap-2';
export const TOGGLE_LABEL = 'text-label-lg text-neutral-40 dark:text-neutral-60';
