// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgFilterList(props: SVGProps<SVGSVGElement>) {
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
                d="M22 6a1 1 0 0 1-1 1H3a1 1 0 1 1 0-2h18a1 1 0 0 1 1 1ZM19 12a1 1 0 0 1-1 1H6a1 1 0 1 1 0-2h12a1 1 0 0 1 1 1ZM16 19a1 1 0 1 0 0-2H8a1 1 0 1 0 0 2h8Z"
            />
        </svg>
    );
}
