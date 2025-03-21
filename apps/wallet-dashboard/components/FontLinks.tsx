// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const ALLIANCE_NO2_FONT_URL = 'https://webassets.iota.org/api/protected?face=alliance-no2';

export function FontLinks() {
    return (
        <link
            rel="stylesheet"
            href={ALLIANCE_NO2_FONT_URL}
            crossOrigin="anonymous"
            integrity="sha384-uKiGwMZQ2tIPdeEsZx9j8cVvrnAbvjJo7yd1IuXpfRnhseLe7V1+qgXWphkKepSb"
        />
    );
}
