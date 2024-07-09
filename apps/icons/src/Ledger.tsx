// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
const SvgLedger = (props: SVGProps<SVGSVGElement>) => (
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
            d="M3 6a2 2 0 0 1 2-2h3a1 1 0 0 1 0 2H5v3a1 1 0 0 1-2 0V6Zm0 12a2 2 0 0 0 2 2h3a1 1 0 1 0 0-2H5v-3a1 1 0 1 0-2 0v3ZM19 4a2 2 0 0 1 2 2v3a1 1 0 1 1-2 0V6h-3a1 1 0 1 1 0-2h3Zm2 14a2 2 0 0 1-2 2h-3a1 1 0 1 1 0-2h3v-3a1 1 0 1 1 2 0v3Zm-9-4V9a1 1 0 1 0-2 0v5a2 2 0 0 0 2 2h2a1 1 0 1 0 0-2h-2Z"
        />
    </svg>
);
export default SvgLedger;
