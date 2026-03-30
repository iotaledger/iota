// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo, TooltipPosition } from '@iota/apps-ui-kit';
import { useCopyToClipboard } from '@iota/core';
import { isString, onCopySuccess } from '~/lib';

/**
 * Defines the structure of a single metadata item.
 */
export interface MetaItem {
    /** The descriptive label of the field (e.g., "Legacy ID", "Type") */
    label: string;
    /**
     * The content to be rendered.
     * - If it's a `string`: Renders plain text + Copy Button.
     * - If it's a `ReactNode`: Renders the component as it is.
     */
    value: string | React.ReactNode;
    /**
     * (Optional) Conditional visibility. If false, the item is filtered before rendering.
     * Useful for cases like "Legacy ID" that don't always exist.
     */
    visible?: boolean;
    /**
     * (Optional) Clarifying text place on tooltip besides label.
     */
    tooltipText?: string;
}

export interface PageHeaderMetaProps {
    items: MetaItem[];
}

export function PageHeaderMeta({ items }: PageHeaderMetaProps) {
    // Filter visible items
    const visibleItems = items.filter((item) => item.visible !== false);

    if (visibleItems.length === 0) return null;

    return (
        <div className="w-1/2">
            {visibleItems.map((item, index) => (
                <MetaItemKV key={index} item={item} />
            ))}
        </div>
    );
}

interface MetaItemKVProps {
    item: MetaItem;
}

function MetaItemKV({ item }: MetaItemKVProps) {
    const copyToClipboard = useCopyToClipboard(onCopySuccess);

    return (
        <>
            {isString(item.value) ? (
                <KeyValueInfo
                    keyText={item.label}
                    copyText={item.value as string}
                    onCopySuccess={() => copyToClipboard(item.value as string)}
                    value={item.value}
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText={item.tooltipText}
                />
            ) : (
                <KeyValueInfo
                    keyText={item.label}
                    value={item.value}
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText={item.tooltipText}
                />
            )}
        </>
    );
}
