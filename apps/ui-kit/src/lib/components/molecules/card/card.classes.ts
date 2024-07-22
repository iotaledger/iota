// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardVariant, ImageType, ImageVariant } from './card.enums';

export const CARD_DISABLED_CLASSES = `cursor-default opacity-40`;

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

export const CARD_CLASSES_VARIANT: Record<CardVariant, string> = {
    [CardVariant.Default]: 'border border-transparent',
    [CardVariant.Outlined]:
        'border border-shader-neutral-light-8 dark:border-shader-primary-dark-8 p-xs',
    [CardVariant.Filled]: 'border border-transparent bg-shader-neutral-light-8 p-xs',
};
