// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgSwapAccount(props: SVGProps<SVGSVGElement>) {
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
                d="M10.707 4.707a1 1 0 0 0-1.414-1.414L4.301 8.285A1.007 1.007 0 0 0 4 9a.995.995 0 0 0 .383.787c.17.134.384.213.617.213h14a1 1 0 1 0 0-2H7.414l3.293-3.293ZM4 16a1 1 0 0 1 1-1h14a.996.996 0 0 1 .876.518.995.995 0 0 1-.08 1.086.997.997 0 0 1-.097.111l-4.992 4.992a1 1 0 0 1-1.414-1.414L16.586 17H5a1 1 0 0 1-1-1Z"
            />
        </svg>
    );
}
