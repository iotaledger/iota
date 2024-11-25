// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgDarkMode(props: SVGProps<SVGSVGElement>) {
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
                d="M11.22 6.05a6.001 6.001 0 1 0 4.317 10.797A7.003 7.003 0 0 1 11.22 6.05M4 12a8 8 0 0 1 8-8c.796 0 1.323.54 1.502 1.122a1.81 1.81 0 0 1-.343 1.677A5 5 0 0 0 17 15a1.39 1.39 0 0 1 1.215.717c.237.43.253 1.034-.155 1.506A8 8 0 0 1 4 12"
                clipRule="evenodd"
            />
        </svg>
    );
}
