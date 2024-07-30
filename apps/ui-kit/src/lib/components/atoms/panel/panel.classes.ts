// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PanelTitleSize } from './panel.enums';

export const PADDING_WITH_TITLE: Record<PanelTitleSize, string> = {
    [PanelTitleSize.Medium]: 'py-md',
    [PanelTitleSize.Large]: 'py-sm',
};

export const PADDING_BOTTOM_TITLE: Record<PanelTitleSize, string> = {
    [PanelTitleSize.Medium]: 'pb-md',
    [PanelTitleSize.Large]: 'pb-xs',
};

export const PADDING_CHILDREN: Record<PanelTitleSize, string> = {
    [PanelTitleSize.Medium]: 'py-xs',
    [PanelTitleSize.Large]: 'py-md',
};
