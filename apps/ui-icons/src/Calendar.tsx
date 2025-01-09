// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgCalendar(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="M14 14a1 1 0 1 0 0 2h2a1 1 0 1 0 0-2z" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M8 2a1 1 0 0 1 1 1v1h6V3a1 1 0 1 1 2 0v1h1a3 3 0 0 1 3 3v10a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3h1V3a1 1 0 0 1 1-1M5 17V8h14v9a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1"
                clipRule="evenodd"
            />
        </svg>
    );
}
