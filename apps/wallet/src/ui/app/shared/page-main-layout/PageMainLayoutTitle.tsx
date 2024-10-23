// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext } from 'react';
import { createPortal } from 'react-dom';
import { PageMainLayoutContext } from './PageMainLayout';
import { Header } from '@iota/apps-ui-kit';

export type PageMainLayoutTitleProps = {
    title: string;
};
export function PageMainLayoutTitle({ title }: PageMainLayoutTitleProps) {
    const titleNode = useContext(PageMainLayoutContext);
    if (titleNode) {
        return createPortal(<Header titleCentered title={title} />, titleNode);
    }
    return null;
}
