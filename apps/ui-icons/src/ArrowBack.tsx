// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowBack(props: SVGProps<SVGSVGElement>) {
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
                d="M4 12.004a.997.997 0 0 0 .293.703l6 6a1 1 0 0 0 1.414-1.414L7.414 13H19a1 1 0 1 0 0-2H7.414l4.293-4.293a1 1 0 0 0-1.414-1.414l-6 6a1 1 0 0 0-.293.705v.006Z"
            />
        </svg>
    );
}
