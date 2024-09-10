// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgSeed(props: SVGProps<SVGSVGElement>) {
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
                d="M12 11a6 6 0 0 0-6-6H5a1 1 0 0 0-1 1v1a6 6 0 0 0 6 6v6a1 1 0 1 0 2 0v-8ZM13 11a6 6 0 0 1 6-6h1a1 1 0 0 1 1 1v1a6 6 0 0 1-6 6h-1a1 1 0 0 1-1-1v-1Z"
            />
        </svg>
    );
}
