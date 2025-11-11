// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function useCookiesManager() {
    const onAcceptCookies = () => {
        document.cookie = `AMP_COOKIES_ACCEPTED=true; max-age=31536000`;
    };
    const onDeclineCookies = () => {
        document.cookie = `AMP_COOKIES_ACCEPTED=false; max-age=31536000`;
    };
    return { onAcceptCookies, onDeclineCookies };
}
