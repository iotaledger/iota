// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const FONT_URL = 'https://webassets.iota.org/api/protected?face=alliance-no2';

export function FontLinkStyle() {
    return <link rel="stylesheet" href={FONT_URL} />;
}
