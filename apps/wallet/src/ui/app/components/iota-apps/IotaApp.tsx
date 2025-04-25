// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon, ImageIconSize } from '@iota/core';
import { ExternalLink } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getDAppUrl } from '_src/shared/utils';
import { useState } from 'react';
import { Card, CardImage, CardBody, ImageShape, Badge, BadgeType } from '@iota/apps-ui-kit';
import { DisconnectApp } from './DisconnectApp';

export type DAppEntry = {
    name: string;
    description: string;
    link: string;
    icon: string;
    tags: string[];
};
export type DisplayType = 'full' | 'card';

interface CardViewProps {
    name: string;
    link: string;
    icon?: string;
}

function CardView({ name, link, icon }: CardViewProps) {
    const appUrl = getDAppUrl(link);
    const originLabel = appUrl.hostname;
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>
                <ImageIcon
                    src={icon || null}
                    label={name}
                    fallback={name}
                    rounded={false}
                    size={ImageIconSize.Small}
                />
            </CardImage>
            <CardBody isTextTruncated title={name} subtitle={originLabel} />
        </Card>
    );
}

interface ListViewProps {
    name: string;
    icon?: string;
    description: string;
    tags?: string[];
}

function ListView({ name, icon, description, tags }: ListViewProps) {
    return (
        <div className="item-center box-border flex gap-sm rounded-2xl bg-neutral-100 p-sm hover:bg-shader-primary-dark-12 dark:bg-neutral-6">
            <ImageIcon src={icon || null} label={name} fallback={name} />
            <div className="flex flex-col justify-center gap-sm">
                <span className="text-label-md text-neutral-10 dark:text-neutral-92">{name}</span>
                <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                    {description}
                </span>
                {tags?.length && (
                    <div className="flex flex-wrap gap-xxs">
                        {tags?.map((tag) => (
                            <Badge key={tag} label={tag} type={BadgeType.Neutral} />
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}

export interface IotaAppProps {
    name: string;
    description: string;
    link: string;
    icon: string;
    tags: string[];
    permissionID?: string;
    displayType: DisplayType;
    openAppSite?: boolean;
}

export function IotaApp({
    name,
    description,
    link,
    icon,
    tags,
    permissionID,
    displayType,
    openAppSite,
}: IotaAppProps) {
    const [showDisconnectApp, setShowDisconnectApp] = useState(false);
    const appUrl = getDAppUrl(link);

    if (permissionID && showDisconnectApp) {
        return (
            <DisconnectApp
                name={name}
                link={link}
                icon={icon}
                permissionID={permissionID}
                setShowDisconnectApp={setShowDisconnectApp}
            />
        );
    }

    const AppDetails =
        displayType === 'full' ? (
            <ListView name={name} description={description} icon={icon} tags={tags} />
        ) : (
            <CardView name={name} link={link} icon={icon} />
        );

    if (permissionID && !openAppSite) {
        return (
            <div onClick={() => setShowDisconnectApp(true)} role="button">
                {AppDetails}
            </div>
        );
    }

    return (
        <ExternalLink
            href={appUrl?.toString() ?? link}
            title={name}
            className="no-underline"
            onClick={() => {
                ampli.openedApplication({ applicationName: name });
            }}
        >
            {AppDetails}
        </ExternalLink>
    );
}
