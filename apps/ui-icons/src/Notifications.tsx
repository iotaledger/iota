// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgNotifications(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M12 2a2 2 0 0 1 2 2v.342c2.33.824 4 3.046 4 5.658v2c0 1.157.172 2.367.925 3.246l.112.131A4 4 0 0 1 20 17.98V19a1 1 0 0 1-1 1h-4.176a3 3 0 0 1-.703 1.121 3 3 0 0 1-4.242 0A3 3 0 0 1 9.176 20H5a1 1 0 0 1-1-1v-1.02a4 4 0 0 1 .963-2.603l.112-.13C5.827 14.366 6 13.156 6 12v-2a6 6 0 0 1 4-5.658V4a2 2 0 0 1 2-2m0 4a4 4 0 0 0-4 4v2c0 1.188-.147 2.873-1.186 4.271l-.22.276-.112.13v.002a2 2 0 0 0-.474 1.123L6 17.98V18h12v-.02c0-.418-.131-.823-.372-1.16l-.11-.141-.112-.132C16.166 15.1 16 13.266 16 12v-2l-.005-.206a4 4 0 0 0-3.789-3.79z"
                clipRule="evenodd"
            />
        </svg>
    );
}
