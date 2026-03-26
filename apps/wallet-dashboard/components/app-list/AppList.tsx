// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import Image from 'next/image';
import { useAppsBackendClient, type AppListItem } from '@iota/apps-backend-client';
import { useQuery } from '@tanstack/react-query';
import { getDefaultNetwork } from '@iota/iota-sdk/client';
import { ExternalLink } from '@/components/ExternalLink';

const AppListItem = (props: AppListItem) => {
    return (
        <ExternalLink
            href={props.link}
            type="application"
            className="flex flex-col items-center hover:opacity-70"
        >
            <div className="relative h-32 w-32 overflow-hidden rounded-md">
                <Image
                    loader={() => props.icon}
                    src={props.icon}
                    alt="Description"
                    className="h-full w-full object-cover"
                    layout={'fill'}
                    objectFit={'contain'}
                />
            </div>
            <h6 className={'mt-2 text-gray-900'}>{props.name}</h6>
            <p className={'mt-3 text-sm text-gray-700'}>{props.description}</p>
        </ExternalLink>
    );
};

export const AppList = () => {
    const client = useAppsBackendClient();

    const { data, isLoading } = useQuery({
        queryKey: ['apps'],
        queryFn: () => client.getApps(getDefaultNetwork()),
    });

    if (isLoading) {
        return <div>Loading...</div>;
    }

    return (
        <div className={'grid grid-cols-5 gap-4'}>
            {data?.apps?.map((app) => {
                return (
                    <div key={app.name} className={'p-3'}>
                        <AppListItem {...app} />
                    </div>
                );
            })}
        </div>
    );
};
