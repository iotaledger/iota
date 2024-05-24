// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppsBackend } from '@mysten/core';
import { useQuery } from '@tanstack/react-query';
import { AppListItem } from '@/components/AppList/AppList.types';

const AppListItem = (props: AppListItem) => {
    return (
        <a
            href={props.link}
            target="_blank"
            rel="noopener noreferrer"
            className={'flex flex-col items-center hover:opacity-70'}
        >
            <div className="h-32 w-32 overflow-hidden rounded-md">
                <img src={props.icon} alt="Description" className="h-full w-full object-cover" />
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
                network: 'MAINNET',
            }),
    });

    if (isLoading) {
        return <div>Loading...</div>;
    }

    return (
        // <div className={'-m-3 flex flex-wrap justify-between'}>
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
