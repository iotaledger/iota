// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/apps-ui-icons';
import {
    Accordion,
    AccordionContent,
    AccordionHeader,
    Badge,
    BadgeType,
    KeyValueInfo,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { useFormatCoin, useGetObject } from '@iota/core';
import type { DisplayFieldsResponse } from '@iota/iota-sdk/client';
import { parseStructTag } from '@iota/iota-sdk/utils';
import { clsx } from 'clsx';
import { type ReactNode, useState } from 'react';
import { ObjectLink } from '~/components/ui';
import { ObjectDisplay } from '~/pages/transaction-result/transaction-summary/ObjectDisplay';

enum ItemLabel {
    Package = 'package',
    Module = 'module',
    Type = 'type',
}

interface ItemProps {
    label: string;
    packageId?: string;
    moduleName?: string;
    typeName?: string;
}

function Item({ label, packageId, moduleName, typeName }: ItemProps): JSX.Element | null {
    switch (label) {
        case ItemLabel.Package:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={<ObjectLink objectId={packageId || ''} copyText={packageId} />}
                />
            );
        case ItemLabel.Module:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={
                        <ObjectLink
                            objectId={packageId ? `${packageId}?module=${moduleName}` : ''}
                            label={moduleName || ''}
                        />
                    }
                />
            );
        case ItemLabel.Type:
            return <KeyValueInfo keyText={label} value={typeName || ''} />;
        default:
            return <KeyValueInfo keyText={label} value="" />;
    }
}

interface ObjectDetailPanelProps {
    panelContent: ReactNode;
    headerContent?: ReactNode;
    hideBorder?: boolean;
}

function ObjectDetailPanel({ panelContent, headerContent }: ObjectDetailPanelProps): JSX.Element {
    const [open, setOpen] = useState(false);
    return (
        <Accordion hideBorder>
            <AccordionHeader hideArrow isExpanded={open} onToggle={() => setOpen(!open)}>
                <div className="flex w-full flex-row items-center justify-between px-md--rs">
                    <div className="flex flex-row gap-xxxs text-iota-neutral-40 dark:text-iota-neutral-60">
                        <span className="text-body-md">Object</span>

                        <TriangleDown
                            className={clsx(
                                'h-5 w-5',
                                open
                                    ? 'rotate-0 transition-transform ease-linear'
                                    : '-rotate-90 transition-transform ease-linear',
                            )}
                        />
                    </div>
                    <div className="flex flex-row items-center gap-xxs overflow-hidden truncate pr-xxs">
                        {headerContent}
                    </div>
                </div>
            </AccordionHeader>
            <AccordionContent isExpanded={open}>{panelContent}</AccordionContent>
        </Accordion>
    );
}

function ObjectDetailBalance({
    objectId,
    typeArg,
}: {
    objectId: string;
    typeArg: string;
}): JSX.Element {
    const { data: objectData, isPending } = useGetObject(objectId);
    const content = objectData?.data?.content;
    const balance =
        content?.dataType === 'moveObject' && content?.fields && 'balance' in content.fields
            ? (content.fields.balance as string)
            : BigInt(0);
    const [formatted, symbol] = useFormatCoin({ balance, coinType: typeArg });

    return isPending ? (
        <div className="mt-1 flex w-full justify-center">
            <LoadingIndicator text="Loading data" />
        </div>
    ) : (
        <KeyValueInfo keyText="Balance" value={formatted} supportingLabel={symbol} />
    );
}

interface ObjectDetailProps {
    objectType: string;
    objectId: string;
    display?: DisplayFieldsResponse;
}

// NOTE: This is the same component declared at */transaction-summary/ObjectChanges.tsx
export function ObjectDetail({
    objectType,
    objectId,
    display,
}: ObjectDetailProps): JSX.Element | null {
    const separator = '::';
    const objectTypeSplit = objectType?.split(separator) || [];
    const typeName = objectTypeSplit.slice(2).join(separator);
    const { address, module, name } = parseStructTag(objectType);

    const objectDetailLabels = [ItemLabel.Package, ItemLabel.Module, ItemLabel.Type];
    const isIotaCoin = typeName?.startsWith('Coin');
    const typeArg = typeName?.match(/<([^>]+)>/)?.[1] || '';

    if (display?.data) return <ObjectDisplay display={display} objectId={objectId} />;

    return (
        <ObjectDetailPanel
            headerContent={
                <div className="flex shrink-0 items-center gap-sm">
                    <Badge type={BadgeType.Neutral} label={name} />
                    {objectId && (
                        <div className="flex flex-col items-end gap-xxxs">
                            <ObjectLink objectId={objectId} />
                        </div>
                    )}
                </div>
            }
            panelContent={
                <div className="flex flex-col gap-xs px-md--rs py-sm--rs pr-16 capitalize">
                    {isIotaCoin && <ObjectDetailBalance objectId={objectId} typeArg={typeArg} />}
                    {objectDetailLabels.map((label) => (
                        <Item
                            key={label}
                            label={label}
                            packageId={address}
                            moduleName={module}
                            typeName={typeName}
                        />
                    ))}
                </div>
            }
        />
    );
}
