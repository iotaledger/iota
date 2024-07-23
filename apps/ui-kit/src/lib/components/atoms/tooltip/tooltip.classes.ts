// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TooltipPosition } from './tooltip.enums';

export const TOOLTIP_POSITION: Record<TooltipPosition, string> = {
    [TooltipPosition.Top]: 'bottom-full left-1/2 transform -translate-x-1/2 mb-1',
    [TooltipPosition.Bottom]: 'top-full left-1/2 transform -translate-x-1/2 mt-1',
    [TooltipPosition.Left]: 'top-1/2 right-full transform -translate-y-1/2 mr-1',
    [TooltipPosition.Right]: 'top-1/2 left-full transform -translate-y-1/2 ml-1',
};
