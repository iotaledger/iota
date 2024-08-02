// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PanelSize } from './panel.enums';

export const PADDING_TOP_WITH_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pt-sm--rs',
    [PanelSize.Small]: 'pt-sm md:pt-md',
};

export const PADDING_BOTTOM_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pb-sm--rs',
    [PanelSize.Small]: 'pb-sm md:pb-md',
};

export const PADDING_TOP_CHILDREN_WITH_TITLE: Record<PanelSize, string> = {
    [PanelSize.Medium]: 'pt-md--rs',
    [PanelSize.Small]: 'pt-xs',
};
