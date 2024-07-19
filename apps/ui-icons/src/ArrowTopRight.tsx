// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowTopRight(props: SVGProps<SVGSVGElement>) {
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
                d="M17.483 6.29a.996.996 0 0 0-.705-.29h-8.07a1 1 0 1 0 0 2h5.656l-8.071 8.071a1 1 0 1 0 1.414 1.414l8.071-8.07v5.656a1 1 0 1 0 2 0v-8.07a1.002 1.002 0 0 0-.291-.706l-.002-.002-.002-.003Z"
            />
        </svg>
    );
}
