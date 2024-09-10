// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, TooltipPosition } from '@iota/apps-ui-kit';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { ArrowUpRight16 } from '@iota/icons';
import { type IotaObjectResponse, type ObjectOwner } from '@iota/iota-sdk/client';
import {
    formatAddress,
    IOTA_TYPE_ARG,
    normalizeStructTag,
    parseStructTag,
} from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { ObjectVideoImage } from '~/components/ui';
import { useResolveVideo } from '~/hooks/useResolveVideo';
import {
    extractName,
    genFileTypeMsg,
    parseImageURL,
    parseObjectType,
    trimStdLibPrefix,
} from '~/lib/utils';

interface HeroVideoImageProps {
    title: string;
    subtitle: string;
    src: string;
    video?: string | null;
}

function HeroVideoImage({ title, subtitle, src, video }: HeroVideoImageProps): JSX.Element {
    const [open, setOpen] = useState(false);

    return (
        <div className="group relative h-full">
            <ObjectVideoImage
                imgFit="contain"
                aspect="square"
                title={title}
                subtitle={subtitle}
                src={src}
                video={video}
                variant="fill"
                open={open}
                setOpen={setOpen}
                rounded="xl"
            />
            <div className="absolute right-3 top-3 hidden h-8 w-8 items-center justify-center rounded-md bg-white/40 backdrop-blur group-hover:flex">
                <ArrowUpRight16 />
            </div>
        </div>
    );
}

interface DescriptionCardProps {
    name?: string | null;
    display?: {
        [key: string]: string;
    } | null;
    objectType: string;
    objectId: string;
}

function DescriptionCard({
    name,
    display,
    objectType,
    objectId,
}: DescriptionCardProps): JSX.Element {
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

    const normalizedStructTag = normalizeStructTag(structTag);
    const objectNameDisplay = name || display?.description;
    const renderDescription = name && display?.description;

    return (
        <div className="flex flex-col gap-md">
            {objectNameDisplay && <DisplayStats label="Name" value={objectNameDisplay} />}
            {renderDescription && <DisplayStats label="Description" value={display.description} />}
            <DisplayStats
                label="Object ID"
                valueLink={`/object/${objectId}`}
                value={formatAddress(objectId)}
            />
            <DisplayStats
                label="Type"
                valueLink={`${address}?module=${module}`}
                value={normalizedStructTag}
                tooltipText={objectType}
                tooltipPosition={TooltipPosition.Right}
            />
        </div>
    );
}

interface VersionCardProps {
    version?: string;
    digest: string;
}

function VersionCard({ version, digest }: VersionCardProps): JSX.Element {
    return (
        <div className="flex flex-col gap-md">
            <DisplayStats label="Version" value={version ?? '--'} />
            <DisplayStats
                label="Last Transaction Block Digest"
                valueLink={`/txblock/${digest}`}
                value={formatAddress(digest)}
            />
        </div>
    );
}

interface OwnerCardProps {
    objOwner?: ObjectOwner | null;
    display?: {
        [key: string]: string;
    } | null;
    storageRebate?: string | null;
}

function OwnerCard({ objOwner, display, storageRebate }: OwnerCardProps): JSX.Element | null {
    const [storageRebateFormatted, symbol] = useFormatCoin(
        storageRebate,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    if (!objOwner && !display) {
        return null;
    }

    function getOwner(objOwner: ObjectOwner): string {
        if (objOwner === 'Immutable') {
            return 'Immutable';
        } else if ('Shared' in objOwner) {
            return 'Shared';
        }
        return 'ObjectOwner' in objOwner
            ? formatAddress(objOwner.ObjectOwner)
            : formatAddress(objOwner.AddressOwner);
    }

    function getOwnerLink(objOwner: ObjectOwner): string | null {
        if (objOwner !== 'Immutable' && !('Shared' in objOwner)) {
            return 'ObjectOwner' in objOwner
                ? `/object/${objOwner.ObjectOwner}`
                : `/address/${objOwner.AddressOwner}`;
        }
        return null;
    }

    return (
        <div className="flex flex-col gap-md">
            {objOwner && (
                <DisplayStats
                    label="Owner"
                    value={getOwner(objOwner)}
                    valueLink={getOwnerLink(objOwner) ?? undefined}
                />
            )}
            {display && display.link && (
                <DisplayStats label="Link" value={display.link} valueLink={display.link} />
            )}
            {display && display.project_url && (
                <DisplayStats
                    label="Website"
                    value={display.project_url}
                    valueLink={display.project_url}
                />
            )}
            <DisplayStats
                label="Storage Rebate"
                value={`-${storageRebateFormatted}`}
                supportingLabel={symbol}
            />
        </div>
    );
}

interface ObjectViewProps {
    data: IotaObjectResponse;
}

export function ObjectView({ data }: ObjectViewProps): JSX.Element {
    const [fileType, setFileType] = useState<undefined | string>(undefined);
    const display = data.data?.display?.data;
    const imgUrl = parseImageURL(display);
    const video = useResolveVideo(data);
    const name = extractName(display);
    const objectType = parseObjectType(data);
    const objOwner = data.data?.owner;
    const storageRebate = data.data?.storageRebate;
    const objectId = data.data?.objectId;
    const lastTransactionBlockDigest = data.data?.previousTransaction;

    const heroImageTitle = name || display?.description || trimStdLibPrefix(objectType);
    const heroImageSubtitle = video ? 'Video' : fileType ?? '';
    const heroImageProps = {
        title: heroImageTitle,
        subtitle: heroImageSubtitle,
        src: imgUrl,
        video: video,
    };

    const { data: imageData } = useQuery({
        queryKey: ['image-file-type', imgUrl],
        queryFn: ({ signal }) => genFileTypeMsg(imgUrl, signal!),
    });

    useEffect(() => {
        if (imageData) {
            setFileType(imageData);
        }
    }, [imageData]);

    return (
        <div className={clsx('address-grid-container-top', !imgUrl && 'no-image')}>
            {imgUrl !== '' && (
                <div style={{ gridArea: 'heroImage' }}>
                    <HeroVideoImage {...heroImageProps} />
                </div>
            )}

            {objectId && (
                <div style={{ gridArea: 'description' }}>
                    <DescriptionCard
                        name={name}
                        objectType={objectType}
                        objectId={objectId}
                        display={display}
                    />
                </div>
            )}

            {lastTransactionBlockDigest && (
                <div style={{ gridArea: 'version' }}>
                    <VersionCard version={data.data?.version} digest={lastTransactionBlockDigest} />
                </div>
            )}

            <div style={{ gridArea: 'owner' }}>
                <OwnerCard objOwner={objOwner} display={display} storageRebate={storageRebate} />
            </div>
        </div>
    );
}
