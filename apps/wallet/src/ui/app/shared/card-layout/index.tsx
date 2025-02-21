// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import { CheckmarkFilled, IotaLogoMark } from '@iota/apps-ui-icons';
import { Header } from '@iota/apps-ui-kit';

export interface CardLayoutProps {
    title?: string;
    subtitle?: string;
    headerCaption?: string;
    icon?: 'success' | 'iota';
    children: ReactNode | ReactNode[];
}

export function CardLayout({ children, title, subtitle, headerCaption, icon }: CardLayoutProps) {
    return (
        <div className="flex max-h-popup-height w-full max-w-popup-width flex-grow flex-col flex-nowrap items-center overflow-auto p-lg">
            {icon === 'success' ? (
                <CheckmarkFilled className="mb-lg h-8 w-8 text-primary-30" />
            ) : null}
            {icon === 'iota' ? (
                <div className="mb-lg flex h-10 w-10 flex-col flex-nowrap items-center justify-center rounded-full bg-primary-30">
                    <IotaLogoMark className="h-6 w-6 text-neutral-100" />
                </div>
            ) : null}
            {headerCaption ? (
                <span className="text-label-sm text-neutral-40">{headerCaption}</span>
            ) : null}
            {title ? (
                <div className="mt-1.25">
                    <Header title={title} titleCentered />
                </div>
            ) : null}
            {subtitle ? (
                <div className="mb-md text-center">
                    <span className="text-label-md text-neutral-10 dark:text-neutral-92">
                        {subtitle}
                    </span>
                </div>
            ) : null}
            {children}
        </div>
    );
}
