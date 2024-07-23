// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgListViewMedium(props: SVGProps<SVGSVGElement>) {
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
                d="M3 7a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Zm4 0H5v2h2V7ZM3 15a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-2Zm4 0H5v2h2v-2Z"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M12 11a1 1 0 1 0 0 2h7a1 1 0 1 0 0-2h-7ZM11 6a1 1 0 0 1 1-1h7a1 1 0 1 1 0 2h-7a1 1 0 0 1-1-1ZM12 17a1 1 0 1 0 0 2h7a1 1 0 1 0 0-2h-7Z"
            />
        </svg>
    );
}
