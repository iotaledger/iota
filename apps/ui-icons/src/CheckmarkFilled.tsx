// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgCheckmarkFilled(props: SVGProps<SVGSVGElement>) {
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
                d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10m4.21-14.104-5.95 5.95-2.147-2.148a.994.994 0 0 0-1.406.009.994.994 0 0 0-.009 1.405l2.846 2.846a.994.994 0 0 0 1.406-.008l6.657-6.657a.994.994 0 0 0 .008-1.406.994.994 0 0 0-1.405.009"
                clipRule="evenodd"
            />
        </svg>
    );
}
