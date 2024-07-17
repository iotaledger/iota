// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardVariant, ImageVariant } from './card.enums';

export const CARD_DISABLED = `cursor-default opacity-40`;

export const IMAGE_SIZE = 'h-[40px] w-[40px]';

export const IMAGE: { [key in ImageVariant]: string } = {
    [ImageVariant.SquareRounded]: `${IMAGE_SIZE} rounded-md`,
    [ImageVariant.Rounded]: `${IMAGE_SIZE} rounded-full`,
};

const CARD_HOVER = `hover:cursor-pointer hover:bg-primary-60 hover:bg-opacity-8`;
export const CARD_CLASSES_VARIANT = {
    [CardVariant.Default]: {
        default: '',
        hover: CARD_HOVER,
        active: `active:cursor-pointer active:bg-primary-60 active:bg-opacity-12`,
    },
    [CardVariant.Outlined]: {
        default: 'border border-shader-neutral-light-8 dark:border-shader-primary-dark-8 p-xs',
        hover: CARD_HOVER,
        active: '',
    },
    [CardVariant.Filled]: {
        default: 'bg-shader-neutral-light-8 p-xs',
        hover: CARD_HOVER,
        active: '',
    },
};
