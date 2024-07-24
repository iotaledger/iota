// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const BACKGROUND_COLORS = 'bg-transparent';
export const BACKGROUND_COLORS_SELECTED = 'bg-primary-100 dark:bg-neutral-6';

const TEXT_COLOR = 'text-neutral-60 dark:text-neutral-40';
const TEXT_COLOR_HOVER = 'enabled:hover:text-neutral-40 enabled:dark:hover:text-neutral-60';
const TEXT_COLOR_FOCUSED = 'enabled:focused:text-neutral-40 enabled:dark:focused:text-neutral-60';

export const TEXT_COLORS = `${TEXT_COLOR} ${TEXT_COLOR_HOVER} ${TEXT_COLOR_FOCUSED}`;
export const TEXT_COLORS_SELECTED = 'text-neutral-10 dark:text-neutral-92';

export const UNDERLINED_SELECTED = `
    relative
    before:content-['']
    border-b
    border-shader-neutral-light-8
    dark:border-shader-neutral-dark-8
    before:block
    before:w-full
    before:border-b
    before:absolute
    before:bottom-[-0.8px]
    before:left-0
    enabled:before:w-[calc(100%-16px)]
    enabled:before:left-2
    enabled:before:border-primary-30
    enabled:before:dark:border-primary-80`;

export const UNDERLINED = `
    relative
    before:content-['']
    before:block
    before:w-full
    before:border-b
    before:border-shader-neutral-light-8
    before:dark:border-shader-neutral-dark-8
    before:absolute
    before:bottom-0
    before:left-0
    hover:before:w-[calc(100%-16px)]
    hover:before:left-2
    hover:before:border-neutral-70
    hover:before:dark:border-neutral-40`;
