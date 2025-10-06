// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function handleOpenInANewTab(pathname: string) {
    let baseUrl = window.location.origin;

    baseUrl = baseUrl.includes('ui.html') ? baseUrl : `${baseUrl}/ui.html`;

    const typeParam = new URLSearchParams({
        type: 'tab',
    });
    baseUrl = `${baseUrl}?${typeParam.toString()}`;

    const finalUrl = `${baseUrl}#${pathname}`;

    window.open(finalUrl, '_blank', 'noopener,noreferrer');
}
