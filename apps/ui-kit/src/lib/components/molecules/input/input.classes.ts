// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const INPUT_CLASSES =
    'w-full bg-transparent text-body-lg caret-input-caret-color focus:outline-none focus-visible:outline-none disabled:cursor-not-allowed';
export const INPUT_TEXT_CLASSES = 'input-text-color dark:input-text-color-dark';
export const INPUT_PLACEHOLDER_CLASSES =
    'placeholder:input-placeholder-color enabled:dark:placeholder:input-placeholder-color';
export const INPUT_NUMBER_CLASSES =
    '[appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none disabled:cursor-not-allowed';
export const BORDER_CLASSES = [
    'px-md py-sm rounded-lg border input-border-color',
    'group-[.enabled]:cursor-text',
    'group-[.errored]:input-border-error-color dark:group-[.errored]:input-border-error-color-dark',
    'hover:group-[.enabled]:input-border-hover-color dark:hover:group-[.enabled]:input-border-hover-color-dark',
    '[&:has(input:focus)]:input-border-focus-color [&:has(input:focus)]:dark:input-border-focus-color-dark',
].join(' ');
export const LABEL_CLASSES =
    'flex flex-col gap-y-2 text-label-lg input-label-color dark:input-label-color-dark';
