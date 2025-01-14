// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TitleSize } from './titleSize.enums';

export const TITLE_PADDINGS: Record<TitleSize, string> = {
    [TitleSize.Small]: 'px-md py-sm--rs',
    [TitleSize.Medium]: 'px-md--rs py-sm--rs',
};

export const TITLE_SIZE: Record<TitleSize, string> = {
    [TitleSize.Small]: 'text-title-md',
    [TitleSize.Medium]: 'text-title-lg',
};
