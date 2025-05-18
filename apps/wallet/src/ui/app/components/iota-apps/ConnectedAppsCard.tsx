// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cn from 'clsx';
import { useConnectedApps } from '../../hooks';
import { Loading, NoData, PageTemplate } from '_components';
import { IotaApp } from './IotaApp';

export function ConnectedAppsCard() {
    const { connectedApps, loading } = useConnectedApps();

    return (
        <PageTemplate title="Connected Apps" isTitleCentered showBackButton>
            <Loading loading={loading}>
                <div
                    className={cn('flex flex-1 flex-col gap-md', {
                        'h-full items-center': !connectedApps?.length,
                    })}
                >
                    {connectedApps.length ? (
                        <div className="flex flex-col gap-xs">
                            {connectedApps.map((app) => (
                                <IotaApp key={app.permissionID} {...app} displayType="card" />
                            ))}
                        </div>
                    ) : (
                        <NoData message="No connected apps found." />
                    )}
                </div>
            </Loading>
        </PageTemplate>
    );
}
