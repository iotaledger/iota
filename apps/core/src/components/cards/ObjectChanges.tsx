// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
    getObjectChangeLabel,
    type ObjectChangesByOwner,
    type ObjectChangeSummary,
    type IotaObjectChangeTypes,
    type IotaObjectChangeWithDisplay,
    ExplorerLinkType,
    useGetDefaultIotaName,
    formatIotaName,
} from '../../';
import { formatAddress } from '@iota/iota-sdk/utils';
import cx from 'clsx';
import { ExpandableList } from '../lists';
import { Collapsible } from '../collapsible';
import { ObjectChangeDisplay } from './ObjectChangeDisplay';
import {
    Badge,
    BadgeType,
    Divider,
    KeyValueInfo,
    Panel,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { TriangleDown } from '@iota/apps-ui-icons';
import { ObjectChange, RenderExplorerLink } from '../../types';
import { NamedAddressTooltip } from '../NamedAddressTooltip';

interface ObjectDetailProps {
    change: IotaObjectChangeWithDisplay;
    ownerKey: string;
    renderExplorerLink: RenderExplorerLink;
    display?: boolean;
}

export function ObjectDetail({ change, renderExplorerLink: ExplorerLink }: ObjectDetailProps) {
    if (change.type === 'transferred' || change.type === 'published') {
        return null;
    }

    const [open, setOpen] = useState(false);
    const [packageId, moduleName, typeName] = change.objectType?.split('<')[0]?.split('::') || [];

    return (
        <Collapsible
            hideBorder
            onOpenChange={(isOpen) => setOpen(isOpen)}
            hideArrow
            render={() => (
                <div className="flex w-full flex-row items-center justify-between">
                    <Title
                        size={TitleSize.Small}
                        title="Object"
                        trailingElement={
                            <TriangleDown
                                className={cx(
                                    'ml-xxxs h-5 w-5 text-iota-neutral-60',
                                    open
                                        ? 'rotate-0 transition-transform ease-linear'
                                        : '-rotate-90 transition-transform ease-linear',
                                )}
                            />
                        }
                    />
                    <div className="flex flex-row items-center gap-xxs pr-md">
                        <Badge type={BadgeType.Neutral} label={typeName} />
                        {change.objectId && (
                            <KeyValueInfo
                                keyText="Package"
                                value={
                                    <ExplorerLink
                                        type={ExplorerLinkType.Object}
                                        objectID={change.objectId}
                                    >
                                        {formatAddress(packageId)}
                                    </ExplorerLink>
                                }
                                fullwidth
                            />
                        )}
                    </div>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm px-md pr-10">
                <KeyValueInfo
                    keyText="Package"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {formatAddress(packageId)}
                        </ExplorerLink>
                    }
                />

                <KeyValueInfo
                    keyText="Module"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {moduleName}
                        </ExplorerLink>
                    }
                />
                <KeyValueInfo
                    keyText="Type"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {typeName}
                        </ExplorerLink>
                    }
                />
            </div>
        </Collapsible>
    );
}

interface ObjectChangeEntryProps {
    type: IotaObjectChangeTypes;
    changes: ObjectChangesByOwner;
    renderExplorerLink: RenderExplorerLink;
}

export function ObjectChangeEntry({
    changes,
    type,
    renderExplorerLink: ExplorerLink,
}: ObjectChangeEntryProps) {
    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => {
                return (
                    <ObjectChangeByOwnerPanel
                        key={`${type}-${owner}`}
                        renderExplorerLink={ExplorerLink}
                        owner={owner}
                        change={changes}
                        type={type}
                    />
                );
            })}
        </>
    );
}

interface ObjectChangesByOwnerPanelProps {
    renderExplorerLink: RenderExplorerLink;
    owner: string;
    change: ObjectChange;
    type: IotaObjectChangeTypes;
}
function ObjectChangeByOwnerPanel({
    type,
    owner,
    change,
    renderExplorerLink,
}: ObjectChangesByOwnerPanelProps) {
    const [open, setOpen] = useState(false);

    const { data: iotaName } = useGetDefaultIotaName(owner);

    const label = getObjectChangeLabel(type);

    const ExplorerLink = renderExplorerLink;
    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm overflow-hidden rounded-xl">
                <Collapsible
                    hideBorder
                    defaultOpen
                    onOpenChange={(isOpen) => {
                        setOpen(isOpen);
                    }}
                    render={() => (
                        <Title
                            size={TitleSize.Small}
                            title="Object Changes"
                            trailingElement={
                                <div className="ml-1 flex">
                                    <Badge type={BadgeType.PrimarySoft} label={label} />
                                </div>
                            }
                        />
                    )}
                >
                    <>
                        {!!change.changesWithDisplay.length && (
                            <div className="flex flex-1 flex-col gap-2 overflow-y-auto">
                                <ExpandableList
                                    initialShowAll={open}
                                    defaultItemsToShow={5}
                                    items={change.changesWithDisplay.map((change) => (
                                        <ObjectChangeDisplay
                                            change={change}
                                            renderExplorerLink={renderExplorerLink}
                                        />
                                    ))}
                                />
                            </div>
                        )}

                        <div className="flex w-full flex-col gap-2">
                            <ExpandableList
                                defaultItemsToShow={5}
                                initialShowAll={open}
                                items={change.changes.map((change) => (
                                    <ObjectDetail
                                        renderExplorerLink={renderExplorerLink}
                                        ownerKey={owner}
                                        change={change}
                                    />
                                ))}
                            />
                        </div>
                    </>
                </Collapsible>
                {owner ? (
                    <div className="flex flex-col gap-y-sm px-md pb-md">
                        <Divider />
                        <KeyValueInfo
                            keyText="Owner"
                            value={
                                <NamedAddressTooltip name={iotaName} address={owner}>
                                    <ExplorerLink type={ExplorerLinkType.Address} address={owner}>
                                        {formatIotaName(iotaName) || formatAddress(owner)}
                                    </ExplorerLink>
                                </NamedAddressTooltip>
                            }
                            fullwidth
                        />
                    </div>
                ) : null}
            </div>
        </Panel>
    );
}

interface ObjectChangesProps {
    changes?: ObjectChangeSummary | null;
    renderExplorerLink: RenderExplorerLink;
}

export function ObjectChanges({ changes, renderExplorerLink }: ObjectChangesProps) {
    if (!changes) return null;

    return (
        <>
            {Object.entries(changes).map(([type, changes]) => {
                return (
                    <ObjectChangeEntry
                        key={type}
                        type={type as keyof ObjectChangeSummary}
                        changes={changes}
                        renderExplorerLink={renderExplorerLink}
                    />
                );
            })}
        </>
    );
}
