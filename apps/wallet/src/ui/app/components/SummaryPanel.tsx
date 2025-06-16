// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title, TitleSize } from '@iota/apps-ui-kit';
import { type ReactNode } from 'react';

interface SummaryPanelProps {
    title: string;
    body: ReactNode;
}

export function SummaryPanel({ title, body }: SummaryPanelProps) {
    return (
        <div
            className={`flex flex-col overflow-y-auto rounded-xl bg-neutral-96 pb-md dark:bg-neutral-12`}
        >
            <div className="flex flex-col gap-y-xs overflow-y-auto">
                <div className="py-2.5">
                    <Title size={TitleSize.Small} title={title} />
                </div>
                {body}
            </div>
        </div>
    );
}
