// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { RenderExplorerLinkProps } from '@iota/core';
import { ExplorerLink } from './explorer-link';

export function ExplorerLinkHelper({ children, ...linkProps }: RenderExplorerLinkProps) {
    return <ExplorerLink {...linkProps}>{children}</ExplorerLink>;
}
