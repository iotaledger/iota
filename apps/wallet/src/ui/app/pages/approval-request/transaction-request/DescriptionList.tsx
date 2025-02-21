// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * TODO: Generalize this file into shared components.
 */

import type { ReactNode } from 'react';

export interface DescriptionItemProps {
    title: string | ReactNode;
    children: ReactNode;
}

export function DescriptionItem({ title, children }: DescriptionItemProps) {
    return (
        <div className="flex items-center">
            <dt className="text-steel-dark text-pBodySmall flex-1 font-medium">{title}</dt>
            <dd className="text-steel-darker text-pBodySmall ml-0 font-medium">{children}</dd>
        </div>
    );
}

export interface DescriptionListProps {
    children: ReactNode;
}

export function DescriptionList({ children }: DescriptionListProps) {
    return <dl className="m-0 flex flex-col gap-2">{children}</dl>;
}
