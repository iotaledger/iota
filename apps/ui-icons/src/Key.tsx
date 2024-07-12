// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgKey(props: SVGProps<SVGSVGElement>) {
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
                d="M6.5 17a5.502 5.502 0 0 0 5.393-4.413l2.4-.087H22v-1c0-1.105-.395-1.5-1.5-1.5h-8.707A5.5 5.5 0 1 0 6.5 17Zm0-2a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z"
                clipRule="evenodd"
            />
            <path fill="currentColor" d="M18 13.5h4v2h-4v-2Z" />
        </svg>
    );
}
