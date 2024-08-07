// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import ExternalLink from '_components/external-link';
import { ampli } from '_src/shared/analytics/ampli';
import { getDAppUrl } from '_src/shared/utils';
import { Text } from '_src/ui/app/shared/text';
import { useState } from 'react';

import DisconnectApp from './DisconnectApp';

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
        <div className="border-gray-45 hover:border-iota/30 hover:bg-iota/10 box-border flex h-32 w-full flex-col rounded-2xl border border-solid bg-white p-3.75">
            <div className="mb-1 flex">
                <ImageIcon src={icon || null} label={name} fallback={name} size="lg" rounded="lg" />
            </div>

            <div className="item-start flex flex-col justify-start gap-1">
                <div className="line-clamp-2 break-all">
                    <Text variant="body" weight="semibold" color="gray-90">
                        {name}
                    </Text>
                </div>
                <Text variant="bodySmall" weight="medium" color="steel" truncate>
                    {originLabel}
                </Text>
            </div>
        </div>
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
        <div className="item-center hover:bg-iota/10 box-border flex gap-3 rounded bg-white px-1.25 py-3.5">
            <ImageIcon src={icon || null} label={name} fallback={name} size="xxl" rounded="lg" />
            <div className="flex flex-col justify-center gap-1">
                <Text variant="body" weight="semibold" color="iota-dark">
                    {name}
                </Text>
                <Text variant="pSubtitle" weight="normal" color="steel-darker">
                    {description}
                </Text>
                {tags?.length && (
                    <div className="mt-0.5 flex flex-wrap gap-1">
                        {tags?.map((tag) => (
                            <div
                                className="item-center border-steel flex justify-center rounded border border-solid px-1.5 py-0.5"
                                key={tag}
                            >
                                <Text variant="captionSmall" weight="medium" color="steel-dark">
                                    {tag}
                                </Text>
                            </div>
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
            <div
                className="w-full cursor-pointer bg-transparent text-left"
                onClick={() => setShowDisconnectApp(true)}
                role="button"
            >
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
