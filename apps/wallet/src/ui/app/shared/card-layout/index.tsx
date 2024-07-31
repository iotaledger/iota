// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '_app/shared/heading';
import { Text } from '_app/shared/text';
import { Iota, ThumbUpFill32 } from '@iota/icons';
import type { ReactNode } from 'react';

export interface CardLayoutProps {
    title?: string;
    subtitle?: string;
    headerCaption?: string;
    icon?: 'success' | 'iota';
    children: ReactNode | ReactNode[];
}

export function CardLayout({ children, title, subtitle, headerCaption, icon }: CardLayoutProps) {
    return (
        <div className="bg-iota-lightest flex max-h-popup-height w-full max-w-popup-width flex-grow flex-col flex-nowrap items-center overflow-auto rounded-20 p-7.5 pt-10 shadow-wallet-content">
            {icon === 'success' ? (
                <div className="border-success mb-2.5 flex h-12 w-12 items-center justify-center rounded-full border-2 border-dotted p-1">
                    <div className="bg-success flex h-8 w-8 items-center justify-center rounded-full">
                        <ThumbUpFill32 className="text-2xl text-white" />
                    </div>
                </div>
            ) : null}
            {icon === 'iota' ? (
                <div className="bg-iota mb-7 flex h-16 w-16 flex-col flex-nowrap items-center justify-center rounded-full">
                    <Iota className="text-4xl text-white" />
                </div>
            ) : null}
            {headerCaption ? (
                <Text variant="caption" color="steel-dark" weight="semibold">
                    {headerCaption}
                </Text>
            ) : null}
            {title ? (
                <div className="mt-1.25 text-center">
                    <Heading
                        variant="heading1"
                        color="gray-90"
                        as="h1"
                        weight="bold"
                        leading="none"
                    >
                        {title}
                    </Heading>
                </div>
            ) : null}
            {subtitle ? (
                <div className="mb-3.75 text-center">
                    <Text variant="caption" color="steel-darker" weight="bold">
                        {subtitle}
                    </Text>
                </div>
            ) : null}
            {children}
        </div>
    );
}
