// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardType, ImageType, ImageShape } from './card.enums';

export const CARD_DISABLED_CLASSES = `cursor-default opacity-40`;

export const IMAGE_SIZE = 'h-10 w-10';

export const IMAGE_VARIANT_CLASSES: { [key in ImageShape]: string } = {
    [ImageShape.SquareRounded]: `${IMAGE_SIZE} rounded-md`,
    [ImageShape.Rounded]: `${IMAGE_SIZE} rounded-full`,
};

export const IMAGE_BG_CLASSES: { [key in ImageType]: string } = {
    [ImageType.Placeholder]: '',
    [ImageType.BgSolid]: 'bg-neutral-96 dark:bg-neutral-10',
    [ImageType.BgWhite]: 'bg-white dark:bg-neutral-6',
    [ImageType.BgTransparent]: '',
};

export const CARD_TYPE_CLASSES: Record<CardType, string> = {
    [CardType.Default]: 'border border-transparent',
    [CardType.Outlined]:
        'border border-shader-neutral-light-8 dark:border-shader-primary-dark-8 p-xs',
    [CardType.Filled]: 'border border-transparent bg-neutral-96 dark:bg-neutral-10 p-xs',
};
