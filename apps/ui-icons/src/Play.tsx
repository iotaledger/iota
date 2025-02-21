// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgPlay(props: SVGProps<SVGSVGElement>) {
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
                d="M19.364 13.118c.848-.497.848-1.739 0-2.236L7.909 4.175C7.061 3.678 6 4.299 6 5.293v13.414c0 .994 1.06 1.615 1.91 1.118z"
            />
        </svg>
    );
}
