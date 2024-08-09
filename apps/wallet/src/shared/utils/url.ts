// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getUrlWithDeviceId } from '_src/shared/analytics/amplitude';

const IOTA_DAPPS = ['iotafrens.com', 'iotans.io'];

export function isValidUrl(url: string | null) {
    if (!url) {
        return false;
    }
    try {
        new URL(url);
        return true;
    } catch (e) {
        return false;
    }
}

export function getDAppUrl(appUrl: string) {
    const url = new URL(appUrl);
    const isIotaDApp = IOTA_DAPPS.includes(url.hostname);
    return isIotaDApp ? getUrlWithDeviceId(url) : url;
}

export function getValidDAppUrl(appUrl: string) {
    try {
        return getDAppUrl(appUrl);
    } catch (error) {
        /* empty */
    }
    return null;
}

export function prepareLinkToCompare(link: string) {
    let adjLink = link.toLowerCase();
    if (!adjLink.endsWith('/')) {
        adjLink += '/';
    }
    return adjLink;
}

/**
 * Includes ? when query string is set
 */
export function toSearchQueryString(searchParams: URLSearchParams) {
    const searchQuery = searchParams.toString();
    if (searchQuery) {
        return `?${searchQuery}`;
    }
    return '';
}
