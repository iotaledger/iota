// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ListItem } from '@iota/apps-ui-kit';

import type { NftDisplayVariants } from '@/components/name-record/variants';

export type MenuListItem = React.ComponentProps<typeof ListItem> & {
    isHidden?: boolean;
};

export type NftDisplayProps = {
    name: string;
    size?: NftDisplayVariants['size'];
    badge?: React.ReactNode;
};
