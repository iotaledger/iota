// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const BACKGROUND_COLOR = 'bg-transparent';
const BACKGROUND_COLORS_HOVERED =
    'hover:bg-shader-primary-light-8 dark:hover:bg-shader-primary-dark-8';
const BACKGROUND_COLORS_FOCUSED =
    'focused:bg-shader-primary-light-12 dark:focused:bg-shader-primary-dark-12';

export const BACKGROUND_COLORS = `${BACKGROUND_COLOR} ${BACKGROUND_COLORS_HOVERED} ${BACKGROUND_COLORS_FOCUSED}`;

const TEXT_COLOR = 'text-neutral-60 dark:text-neutral-40';
const TEXT_COLOR_HOVER = 'hover:text-neutral-40 dark:hover:text-neutral-60';
const TEXT_COLOR_FOCUSED = 'focused:text-neutral-40 dark:focused:text-neutral-60';

export const TEXT_COLORS = `${TEXT_COLOR} ${TEXT_COLOR_HOVER} ${TEXT_COLOR_FOCUSED}`;
export const TEXT_SELECTED = 'text-neutral-10 dark:text-neutral-6';
