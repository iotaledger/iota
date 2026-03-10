// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy } from '@iota/apps-ui-icons';

import { copyToClipboard } from '@/lib/utils/copyToClipboard';

interface NameCardBodyProps {
    name: string;
    valueToCopy?: string;
}

export function NameCardBody({
    name,
    valueToCopy = name,
    children,
}: React.PropsWithChildren<NameCardBodyProps>) {
    function handleCopy() {
        copyToClipboard(valueToCopy);
    }

    return (
        <div className="p-md flex flex-col gap-y-xs" data-testid="name-card-body">
            <div
                onClick={handleCopy}
                className="group flex items-center gap-x-xs text-names-neutral-92 text-title-md truncate overflow-hidden cursor-pointer"
                role="button"
                aria-label="Copy to clipboard"
            >
                <span className="truncate cursor-pointer">{name}</span>

                <Copy className="w-4 h-4 opacity-0 transition-opacity group-hover:opacity-100 text-names-neutral-92 shrink-0" />
            </div>

            <div className="flex flex-col gap-y-xs">{children}</div>
        </div>
    );
}
