// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import PageTitle, { type PageTitleProps } from '_src/ui/app/shared/PageTitle';
import { type ReactNode } from 'react';

export interface MenuLayoutProps extends PageTitleProps {
    children: ReactNode;
}

export function MenuLayout({ children, ...pageTitleProps }: MenuLayoutProps) {
    return (
        <>
            <div className="sticky top-0 bg-white py-4">
                <PageTitle {...pageTitleProps} />
            </div>
            <div className="flex flex-1 flex-col justify-items-stretch px-2.5">{children}</div>
        </>
    );
}
