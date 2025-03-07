// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgVesting(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <g fill="currentColor" clipPath="url(#vesting_svg__a)">
                <path d="M8 2a1 1 0 0 1 1 1v1h6V3a1 1 0 1 1 2 0v1h1a3 3 0 0 1 3 3v6a1 1 0 1 1-2 0V8H5v9a1 1 0 0 0 1 1h4a1 1 0 1 1 0 2H6a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3h1V3a1 1 0 0 1 1-1" />
                <path d="M8 10a1 1 0 1 0 0 2h2a1 1 0 1 0 0-2z" />
                <path
                    fillRule="evenodd"
                    d="M18 22.465a4 4 0 1 1 0-6.929 4 4 0 1 1 0 6.929m1.5-1.528a2.002 2.002 0 0 0 2.483-1.676 2 2 0 0 0-2.482-2.198c.318.574.499 1.234.499 1.937s-.181 1.363-.5 1.937"
                    clipRule="evenodd"
                />
            </g>
            <defs>
                <clipPath id="vesting_svg__a">
                    <path fill="#fff" d="M0 0h24v24H0z" />
                </clipPath>
            </defs>
        </svg>
    );
}
