// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider } from '@iota/apps-ui-kit';

interface SectionHeaderProps {
    title: string;
}

export function SectionHeader({ title }: SectionHeaderProps) {
    return (
        <div className="flex items-center justify-center gap-md">
            <div className="text-body-md text-neutral-60">{title}</div>
            <div className=" flex h-px flex-1 flex-shrink-0">
                <Divider />
            </div>
        </div>
    );
}
