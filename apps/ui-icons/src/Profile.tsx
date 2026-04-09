// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgProfile(props: SVGProps<SVGSVGElement>) {
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
                d="M12 2c5.523 0 10 4.477 10 10s-4.477 10-10 10S2 17.523 2 12 6.477 2 12 2m0 2a8 8 0 0 0-5.809 13.498 8 8 0 0 1 11.617 0A7.97 7.97 0 0 0 20 12a8 8 0 0 0-8-8"
                clipRule="evenodd"
            />
            <path fill="currentColor" d="M15 10a3 3 0 1 1-6 0 3 3 0 0 1 6 0" />
        </svg>
    );
}
