// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
const SvgQrCode = (props: SVGProps<SVGSVGElement>) => (
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
            d="M5 3a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2H5Zm4 2H5v4h4V5Zm-4 8a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2H5Zm4 2H5v4h4v-4Zm4-10a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2V5Zm2 0h4v4h-4V5Z"
            clipRule="evenodd"
        />
        <path
            fill="currentColor"
            d="M21 14a1 1 0 1 1-2 0 1 1 0 0 1 2 0Zm-7 7a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm-1-6a2 2 0 0 1 2-2h1a1 1 0 1 1 0 2h-1v1a1 1 0 1 1-2 0v-1Zm6 6a2 2 0 0 0 2-2v-1a1 1 0 1 0-2 0v1h-1a1 1 0 1 0 0 2h1Z"
        />
    </svg>
);
export default SvgQrCode;
