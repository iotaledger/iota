// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getUrlWithDeviceId } from '../analytics/amplitude';

const IOTA_DAPPS: string[] = [];

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

/**
 * Extracts application name from URL hostname
 * Removes 'www.' prefix and returns first part of domain
 * Returns empty string if URL is invalid
 */
export function getAppNameFromOrigin(origin: string): string {
    try {
        return new URL(origin).hostname.replace('www.', '').split('.')[0];
    } catch (e) {
        return '';
    }
}

export interface EcosystemApp {
    link: string;
    name: string;
    [key: string]: unknown;
}

/**
 * Resolves application name by matching with ecosystem apps or parsing from origin
 * Prioritizes provided name, then matched ecosystem app name, then parsed origin name
 */
export function resolveApplicationName(
    permissionName: string | undefined,
    origin: string,
    pagelink: string | undefined,
    ecosystemApps: EcosystemApp[],
): string {
    // Try to match with ecosystem apps
    const matchedEcosystemApp = ecosystemApps.find((app) => {
        const originAdj = prepareLinkToCompare(origin);
        const pagelinkAdj = pagelink ? prepareLinkToCompare(pagelink) : null;
        const appLinkAdj = prepareLinkToCompare(app.link);
        return originAdj === appLinkAdj || pagelinkAdj === appLinkAdj;
    });

    // Return in priority order: permission name > ecosystem app name > parsed origin name
    if (permissionName) {
        return permissionName;
    }
    if (matchedEcosystemApp?.name) {
        return matchedEcosystemApp.name;
    }
    return getAppNameFromOrigin(origin);
}
