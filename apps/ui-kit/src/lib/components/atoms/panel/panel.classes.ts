// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PanelSize } from './panel.enums';

export const PADDING_TOP_WITH_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pt-xs md:pt-sm',
    [PanelSize.Small]: 'pt-sm md:pt-md',
};

export const PADDING_BOTTOM_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pb-xs md:pb-sm',
    [PanelSize.Small]: 'pb-sm md:pb-md',
};

export const PADDING_TOP_CHILDREN_WITH_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pt-md md:pt-lg',
    [PanelSize.Small]: 'pt-xs',
};
