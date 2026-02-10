// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ToggleSize } from './toggle.enums';

export const TOGGLE = 'relative inline-flex items-center p-xxs border rounded-full cursor-pointer';

export const TOGGLE_STATES = {
    active: 'toggle-bg-active toggle-border-active',
    inactive: 'toggle-bg-inactive toggle-border-inactive',
    disabledActive: 'toggle-bg-disabled-active toggle-border-disabled-active',
    disabled: 'opacity-40 cursor-not-allowed',
};

export const TOGGLE_SIZE = {
    [ToggleSize.Small]: 'h-5 w-10',
    [ToggleSize.Default]: 'h-6 w-12',
};

export const TOGGLE_THUMB = 'absolute rounded-full transition-all duration-200 ease-in-out';
export const TOGGLE_THUMB_POSITION = {
    [ToggleSize.Small]: {
        unchecked: 'left-1',
        checked: 'left-[34px] -translate-x-full',
    },
    [ToggleSize.Default]: {
        unchecked: 'left-1',
        checked: 'left-[42px] -translate-x-full',
    },
};
export const TOGGLE_THUMB_COLOR = {
    unchecked: 'toggle-thumb-color',
    checked: 'toggle-thumb-color-checked',
};
export const TOGGLE_THUMB_SIZE = {
    [ToggleSize.Small]: 'h-3 w-3',
    [ToggleSize.Default]: 'h-4 w-4',
};

export const TOGGLE_CONTAINER = 'inline-flex items-center gap-2';
export const TOGGLE_LABEL = 'text-label-lg toggle-label-color';
