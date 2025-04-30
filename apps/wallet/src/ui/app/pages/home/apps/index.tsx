// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeature } from '@growthbook/growthbook-react';
import { FiltersPortal, ConnectedAppsCard, type DAppEntry, AppsPlayGround } from '_components';
import { getFromSessionStorage, setToSessionStorage } from '_src/background/storageUtils';
import { Feature } from '@iota/core';
import { useUnlockedGuard } from '_hooks';
import { useEffect } from 'react';
import { Route, Routes, useNavigate } from 'react-router-dom';

import st from './AppsPage.module.scss';

const APPS_PAGE_NAVIGATION = 'APPS_PAGE_NAVIGATION';

type FilterTag = {
    name: string;
    link: string;
};

export function AppsPage() {
    const navigate = useNavigate();

    const DEFAULT_FILTER_TAGS: FilterTag[] = [
        {
            name: 'Connections',
            link: 'apps/connected',
        },
        {
            name: 'All',
            link: 'apps',
        },
    ];
    const ecosystemApps = useFeature<DAppEntry[]>(Feature.WalletDapps).value ?? [];

    const uniqueAppTags = Array.from(new Set(ecosystemApps.flatMap((app) => app.tags)))
        .map((tag) => ({
            name: tag,
            // The tag subroute is used to get around the NavLink limitation with reading query params
            // Enables active route highlighting without excessive overhead
            link: `apps/${tag}`,
        }))
        .sort((a, b) => a.name.localeCompare(b.name));

    const allFilterTags = [...DEFAULT_FILTER_TAGS, ...uniqueAppTags];

    useEffect(() => {
        getFromSessionStorage<string>(APPS_PAGE_NAVIGATION).then((activeTagLink) => {
            if (activeTagLink) {
                navigate(`/${activeTagLink}`);

                const element = document.getElementById(activeTagLink);

                if (element) {
                    element.scrollIntoView();
                }
            }
        });
    }, [navigate]);

    const handleFiltersPortalClick = async (tag: FilterTag) => {
        await setToSessionStorage<string>(APPS_PAGE_NAVIGATION, tag.link);
    };

    if (useUnlockedGuard()) {
        return null;
    }

    return (
        <div className={st.container} data-testid="apps-page">
            <FiltersPortal
                firstLastMargin
                tags={allFilterTags}
                callback={handleFiltersPortalClick}
            />
            <Routes>
                <Route path="/:tagName?" element={<AppsPlayGround />} />
                <Route path="/connected" element={<ConnectedAppsCard />} />
            </Routes>
        </div>
    );
}
