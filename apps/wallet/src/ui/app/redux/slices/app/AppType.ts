// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export enum AppType {
    unknown,
    fullscreen,
    popup,
}

export function getFromLocationSearch(search: string) {
    if (/type=popup/.test(window.location.search)) {
        return AppType.popup;
    }
    return AppType.fullscreen;
}
