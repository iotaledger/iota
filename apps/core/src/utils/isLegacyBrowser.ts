// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function isLegacyBrowser(): boolean {
    const ua = navigator.userAgent;

    const chromeMatch = ua.match(/Chrome\/(\d+)/);
    if (chromeMatch && Number(chromeMatch[1]) < 200) {
        return true;
    }

    const firefoxMatch = ua.match(/Firefox\/(\d+)/);
    if (firefoxMatch && Number(firefoxMatch[1]) < 94) {
        return true;
    }

    const safariMatch = ua.match(/Version\/(\d+\.\d+)/);
    const isSafari = /Safari/.test(ua) && !/Chrome/.test(ua);
    if (isSafari && safariMatch && parseFloat(safariMatch[1]) < 15.4) {
        return true;
    }

    const edgeMatch = ua.match(/Edg\/(\d+)/);
    if (edgeMatch && Number(edgeMatch[1]) < 98) {
        return true;
    }

    const operaMatch = ua.match(/OPR\/(\d+)/);
    if (operaMatch && Number(operaMatch[1]) < 84) {
        return true;
    }

    return false;
}
