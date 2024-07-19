// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardVariant, ImageType, ImageVariant } from './card.enums';

export const CARD_DISABLED = `cursor-default opacity-40`;

export const IMAGE_SIZE = 'h-10 w-10';

export const IMAGE_VARIANT_CLASSES: { [key in ImageVariant]: string } = {
    [ImageVariant.SquareRounded]: `${IMAGE_SIZE} rounded-md`,
    [ImageVariant.Rounded]: `${IMAGE_SIZE} rounded-full`,
};

export const IMAGE_BG_CLASSES: { [key in ImageType]: string } = {
    [ImageType.Placeholder]: ``,
    [ImageType.BgSolid]: `bg-neutral-96`,
    [ImageType.BgTransparent]: ``,
};

const CARD_HOVER = `hover:cursor-pointer hover:bg-primary-60 hover:bg-opacity-8 hover:dark:bg-primary-60 hover:dark:bg-opacity-8`;
const CARD_ACTIVE = `active:cursor-pointer active:bg-primary-60 active:bg-opacity-12 dark:active:bg-primary-60 dark:active:bg-opacity-12`;
export const CARD_CLASSES_VARIANT = {
    [CardVariant.Default]: {
        default: '',
        hover: CARD_HOVER,
        active: CARD_ACTIVE,
    },
    [CardVariant.Outlined]: {
        default: 'border border-shader-neutral-light-8 dark:border-shader-primary-dark-8 p-xs',
        hover: CARD_HOVER,
        active: CARD_ACTIVE,
    },
    [CardVariant.Filled]: {
        default: 'bg-shader-neutral-light-8 p-xs',
        hover: CARD_HOVER,
        active: CARD_ACTIVE,
    },
};
