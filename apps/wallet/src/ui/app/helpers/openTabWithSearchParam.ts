// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export async function openTabWithSearchParam(searchParam: string, searchParamValue: string) {
    const currentURL = new URL(window.location.href);
    const [currentHash, currentHashSearch] = currentURL.hash.split('?');
    const urlSearchParams = new URLSearchParams(currentHashSearch);
    urlSearchParams.set(searchParam, searchParamValue);
    currentURL.hash = `${currentHash}?${urlSearchParams.toString()}`;
    currentURL.searchParams.delete('type');
    await Browser.tabs.create({
        url: currentURL.href,
    });
}
