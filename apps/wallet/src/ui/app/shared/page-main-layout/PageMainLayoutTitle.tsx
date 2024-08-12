// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext } from 'react';
import { createPortal } from 'react-dom';

import { Heading } from '_app/shared/heading';
import { PageMainLayoutContext } from './PageMainLayout';

export type PageMainLayoutTitleProps = {
    title: string;
};
export function PageMainLayoutTitle({ title }: PageMainLayoutTitleProps) {
    const titleNode = useContext(PageMainLayoutContext);
    if (titleNode) {
        return createPortal(
            <Heading variant="heading4" truncate weight="semibold" color="gray-90">
                {title}
            </Heading>,
            titleNode,
        );
    }
    return null;
}
