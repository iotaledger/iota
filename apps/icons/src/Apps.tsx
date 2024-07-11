// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
const SvgApps = (props: SVGProps<SVGSVGElement>) => (
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
            d="M5 3a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2H5Zm4 2H5v4h4V5Z"
            clipRule="evenodd"
        />
        <path
            fill="currentColor"
            d="M17 3a1 1 0 0 1 1 1v2h2a1 1 0 1 1 0 2h-2v2a1 1 0 1 1-2 0V8h-2a1 1 0 1 1 0-2h2V4a1 1 0 0 1 1-1Z"
        />
        <path
            fill="currentColor"
            fillRule="evenodd"
            d="M5 13a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5Zm4 2H5v4h4v-4Zm4 0a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2v-4Zm2 4v-4h4v4h-4Z"
            clipRule="evenodd"
        />
    </svg>
);
export default SvgApps;
