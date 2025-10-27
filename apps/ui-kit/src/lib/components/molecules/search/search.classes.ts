// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SearchBarType } from './search.enums';

export const BACKGROUND_COLORS: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]: 'bg-transparent',
    [SearchBarType.Filled]: 'search-bg-color-filled',
};

export const SEARCH_WRAPPER_STYLE: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]: 'border-l border-r border-t search-border-color',
    [SearchBarType.Filled]: 'search-bg-color-filled',
};

export const SUGGESTIONS_WRAPPER_STYLE: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]:
        'rounded-b-3xl border-b border-l border-r search-border-color search-suggestion-bg-color',
    [SearchBarType.Filled]: 'rounded-b-3xl search-bg-color-filled',
};
