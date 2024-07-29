// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Resize } from './text-field.enums';

export const INPUT_CLASSES =
    'w-full bg-transparent text-body-lg caret-primary-30 focus:outline-none focus-visible:outline-none';

export const INPUT_TEXT_CLASSES = 'text-neutral-10 dark:text-neutral-92';
export const PLACEHOLDER_TEXT_CLASSES =
    ' enabled:placeholder:text-neutral-40/40  dark:placeholder:text-neutral-60/40 enabled:dark:placeholder:text-neutral-60/40';
export const BORDER_CLASSES =
    'px-md py-sm rounded-lg border border-neutral-80 group-[.enabled]:cursor-text group-[.errored]:border-error-30 hover:group-[.enabled]:border-neutral-50  dark:border-neutral-60 dark:hover:border-neutral-60 dark:group-[.errored]:border-error-80 [&:has(input:focus)]:border-primary-30';

export const RESIZE_CLASSES: Record<Resize, string> = {
    [Resize.None]: 'resize-none',
    [Resize.Both]: 'resize',
    [Resize.Horizontal]: 'resize-x',
    [Resize.Vertical]: 'resize-y',
};
