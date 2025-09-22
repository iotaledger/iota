// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, TooltipPosition } from '@iota/apps-ui-kit';
import { capitalize, resolveNFTMedia, useFormatCoin, useNFTMediaHeaders } from '@iota/core';
import { type IotaObjectResponse, type ObjectOwner } from '@iota/iota-sdk/client';
import {
    CoinFormat,
    formatAddress,
    formatDigest,
    formatType,
    normalizeStructTag,
    parseStructTag,
} from '@iota/iota-sdk/utils';
import { SortByDefault } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { type ReactNode, useState } from 'react';
import { AddressLink, Link, ObjectLink, ObjectVideoImage, TransactionLink } from '~/components/ui';
import { extractName, onCopySuccess, parseObjectType, trimStdLibPrefix } from '~/lib/utils';

interface HeroVideoImageProps {
    title: string;
    subtitle: string;
    src: string;
}

function HeroVideoImage({ title, subtitle, src }: HeroVideoImageProps): JSX.Element {
    const [open, setOpen] = useState(false);

    return (
        <div className="group relative h-full">
            <ObjectVideoImage
                objectFit="contain"
                title={title}
                subtitle={subtitle}
                src={src}
                variant="fill"
                open={open}
                setOpen={setOpen}
                rounded="xl"
                disableVideoControls
            />
            <Link
                href={src}
                target="_blank"
                rel="noopener noreferrer"
                className="absolute right-3 top-3 hidden h-8 w-8 items-center justify-center rounded-md bg-white/40 backdrop-blur group-hover:flex"
            >
                <SortByDefault className="h-4 w-4 rotate-45" />
            </Link>
        </div>
    );
}

interface NameCardProps {
    name: string;
}

function NameCard({ name }: NameCardProps): JSX.Element {
    return <DisplayStats label="Name" value={name} />;
}

interface DescriptionCardProps {
    display?: {
        [key: string]: string;
    };
}

function DescriptionCard({ display }: DescriptionCardProps): JSX.Element {
    return <DisplayStats label="Description" value={display?.description ?? ''} />;
}

interface ObjectIdCardProps {
    objectId: string;
}

function ObjectIdCard({ objectId }: ObjectIdCardProps): JSX.Element {
    return (
        <DisplayStats
            label="Object ID"
            value={
                <div className="flex flex-col gap-xs">
                    <ObjectLink objectId={objectId} copyText={objectId} />
                </div>
            }
        />
    );
}

interface TypeCardCardProps {
    objectType: string;
}

function TypeCard({ objectType }: TypeCardCardProps): JSX.Element {
    const { address, module, typeParams, ...rest } = parseStructTag(objectType);

    const formattedTypeParams = typeParams.map((typeParam) => {
        if (typeof typeParam === 'string') {
            return typeParam;
        } else {
            return {
                ...typeParam,
                address: formatAddress(typeParam.address),
            };
        }
    });

    const structTag = {
        address: formatAddress(address),
        module,
        typeParams: formattedTypeParams,
        ...rest,
    };

    const normalizedStructTag = formatType(normalizeStructTag(structTag));
    return (
        <DisplayStats
            label="Type"
            value={
                <ObjectLink objectId={`${address}?module=${module}`} label={normalizedStructTag}>
                    {normalizedStructTag}
                </ObjectLink>
            }
            tooltipText={objectType}
            tooltipPosition={TooltipPosition.Top}
            copyText={objectType}
            onCopySuccess={onCopySuccess}
        />
    );
}

interface VersionCardProps {
    version?: string;
}

function VersionCard({ version }: VersionCardProps): JSX.Element {
    return <DisplayStats label="Version" value={version ?? '--'} />;
}

interface LastTxBlockCardProps {
    digest: string;
}

function LastTxBlockCard({ digest }: LastTxBlockCardProps): JSX.Element {
    return (
        <DisplayStats
            label="Last Transaction Block Digest"
            value={<TransactionLink digest={digest}>{formatDigest(digest)}</TransactionLink>}
            copyText={digest}
            onCopySuccess={onCopySuccess}
        />
    );
}

function getOwnerDisplay(objOwner: ObjectOwner): 'Shared' | 'Immutable' | string {
    if (objOwner === 'Immutable') {
        return 'Immutable';
    } else if ('Shared' in objOwner) {
        return 'Shared';
    }
    return 'ObjectOwner' in objOwner ? objOwner.ObjectOwner : objOwner.AddressOwner;
}

interface OwnerCardProps {
    objOwner: ObjectOwner;
}

function OwnerCard({ objOwner }: OwnerCardProps): JSX.Element | null {
    return (
        <DisplayStats
            label="Owner"
            value={
                <div className="flex flex-col gap-xs">
                    <OwnerDisplay objOwner={objOwner} />
                </div>
            }
        />
    );
}

function OwnerDisplay({ objOwner }: { objOwner: ObjectOwner }): ReactNode {
    const owner = getOwnerDisplay(objOwner);
    if (objOwner !== 'Immutable' && !('Shared' in objOwner)) {
        if ('ObjectOwner' in objOwner) {
            return <ObjectLink objectId={objOwner.ObjectOwner} copyText={objOwner.ObjectOwner} />;
        } else {
            return <AddressLink address={objOwner.AddressOwner} copyText={objOwner.AddressOwner} />;
        }
    }

    return <span className="text-iota-neutral-10 dark:text-iota-neutral-92">{owner}</span>;
}

interface StorageRebateCardProps {
    storageRebate: string;
}

function StorageRebateCard({ storageRebate }: StorageRebateCardProps): JSX.Element | null {
    const [storageRebateFormatted, symbol] = useFormatCoin({
        balance: storageRebate,
        format: CoinFormat.Full,
    });

    return (
        <DisplayStats
            label="Storage Rebate"
            value={`-${storageRebateFormatted}`}
            supportingLabel={symbol}
        />
    );
}

interface ObjectViewProps {
    data: IotaObjectResponse;
}

export function ObjectView({ data }: ObjectViewProps): JSX.Element {
    const display = data.data?.display?.data;
    const src = display?.image_url || '';
    const { data: nftMediaHeaders } = useNFTMediaHeaders(src);
    const { type: nftFileType } = resolveNFTMedia(src, nftMediaHeaders);

    const name = extractName(display);
    const objectType = parseObjectType(data);
    const objOwner = data.data?.owner;
    const storageRebate = data.data?.storageRebate;
    const objectId = data.data?.objectId;
    const lastTransactionBlockDigest = data.data?.previousTransaction;

    const heroImageTitle = name || display?.description || trimStdLibPrefix(objectType);
    const heroImageSubtitle = `1 ${capitalize(nftFileType)} File`;
    const heroImageProps = {
        title: heroImageTitle,
        subtitle: heroImageSubtitle,
        src,
    };

    return (
        <div className="flex flex-col gap-md">
            <div
                className={clsx(
                    'address-grid-container-top',
                    !src && 'no-image',
                    (!name || !display) && 'no-description',
                )}
            >
                {src && (
                    <div style={{ gridArea: 'heroImage' }}>
                        <HeroVideoImage {...heroImageProps} />
                    </div>
                )}
                {name && (
                    <div style={{ gridArea: 'name' }}>
                        <NameCard name={name} />
                    </div>
                )}
                {display?.description && (
                    <div style={{ gridArea: 'description' }}>
                        <DescriptionCard display={display} />
                    </div>
                )}

                {objectId && (
                    <div style={{ gridArea: 'objectId' }}>
                        <ObjectIdCard objectId={objectId} />
                    </div>
                )}

                {objectType && objectType !== 'unknown' && (
                    <div style={{ gridArea: 'type' }}>
                        <TypeCard objectType={objectType} />
                    </div>
                )}

                {data.data?.version && (
                    <div style={{ gridArea: 'version' }}>
                        <VersionCard version={data.data?.version} />
                    </div>
                )}
                {lastTransactionBlockDigest && (
                    <div style={{ gridArea: 'lastTxBlock' }}>
                        <LastTxBlockCard digest={lastTransactionBlockDigest} />
                    </div>
                )}
                {objOwner && (
                    <div style={{ gridArea: 'owner' }}>
                        <OwnerCard objOwner={objOwner} />
                    </div>
                )}
                {storageRebate && (
                    <div style={{ gridArea: 'storageRebate' }}>
                        <StorageRebateCard storageRebate={storageRebate} />
                    </div>
                )}
            </div>
            <div className="flex flex-row gap-md">
                {display && display.link && (
                    <div className="flex-1">
                        <DisplayStats
                            label="Link"
                            value={<Link href={display.link}>{display.link}</Link>}
                        />
                    </div>
                )}
                {display && display.project_url && (
                    <div className="flex-1">
                        <DisplayStats
                            label="Website"
                            value={<Link href={display.project_url}>{display.project_url}</Link>}
                        />
                    </div>
                )}
            </div>
        </div>
    );
}
