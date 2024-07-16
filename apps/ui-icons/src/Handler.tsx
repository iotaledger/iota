// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgHandler(props: SVGProps<SVGSVGElement>) {
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
                d="M22.32 11.46c.407-.409 1.093-.132 1.087.439l-.082 6.778a.66.66 0 0 1-.648.648l-6.778.082c-.57.007-.848-.68-.44-1.088l6.86-6.86Z"
            />
        </svg>
    );
}
