// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Image from 'next/image';
import { useAppsBackend } from '@iota/core';
import { useQuery } from '@tanstack/react-query';
import { type AppListItem } from './appList.types';
import { getDefaultNetwork } from '@iota/iota-sdk/client';

const AppListItem = (props: AppListItem) => {
    return (
        <a
            href={props.link}
            target="_blank"
            rel="noopener noreferrer"
            className={'flex flex-col items-center hover:opacity-70'}
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
        </a>
    );
};

export const AppList = () => {
    const { request } = useAppsBackend();

    const { data, isLoading } = useQuery<{
        status: number;
        apps: AppListItem[];
        dataUpdated: string;
    }>({
        queryKey: ['apps'],
        queryFn: () =>
            request('api/features/apps', {
                network: getDefaultNetwork(),
            }),
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
