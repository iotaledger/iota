// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function openInNewTab(url: string) {
    const queryParams = new URLSearchParams(window.location.search);
    queryParams.set('type', 'tab');

    url = url.startsWith('/ui.html') ? url.replace('/ui.html', '') : url;
    url = url + (queryParams.toString() ? `?${queryParams}` : '');

    const finalUrl = `${window.location.origin}/ui.html#${url.startsWith('/') ? url : `/${url}`}`;

    window.open(finalUrl, '_blank', 'noopener noreferrer');
}
