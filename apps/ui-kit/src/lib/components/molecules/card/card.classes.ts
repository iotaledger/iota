// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageVariant } from './card.enums';

export const CARD_HOVER = `hover:cursor-pointer hover:bg-primary-60 hover:bg-opacity-8`;
export const CARD_ACTIVE = `active:cursor-pointer active:bg-primary-60 active:bg-opacity-12`;
export const CARD_DISABLED = `cursor-default opacity-40`;

export const IMAGE_SIZE = 'h-[40px] w-[40px]';

export const IMAGE: { [key in ImageVariant]: string } = {
    [ImageVariant.SquareRounded]: `${IMAGE_SIZE} rounded-md`,
    [ImageVariant.Rounded]: `${IMAGE_SIZE} rounded-full`,
};
