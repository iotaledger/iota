// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SearchBarType } from './search.enums';

export const BACKGROUND_COLORS: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]: 'bg-transparent',
    [SearchBarType.Filled]: 'bg-neutral-92 dark:bg-neutral-12',
};

export const SEARCH_WRAPPER_STYLE: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]: 'border-l border-r border-t border-neutral-70 dark:border-neutral-40',
    [SearchBarType.Filled]: 'bg-neutral-92 dark:bg-neutral-12',
};

export const SUGGESTIONS_WRAPPER_STYLE: Record<SearchBarType, string> = {
    [SearchBarType.Outlined]:
        'rounded-b-3xl border-b border-l border-r border-neutral-70  dark:border-neutral-40',
    [SearchBarType.Filled]: 'rounded-b-3xl bg-neutral-92 dark:bg-neutral-12',
};
