// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgSidepanel(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 20 20"
            {...props}
        >
            <path fill="currentColor" d="m10.833 10-4.166 3.333V6.667z" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M16.345 2.5c1.103 0 2 .896 2 2l-.012 11.044c0 1.103-.897 1.956-2 1.956l-12.666-.045c-1.103 0-2-.852-2-1.955l.012-11.045c0-1.103.896-2 2-2zM3.333 15.832H12.5V4.166H3.333zm10.834 0h2.5V4.166h-2.5z"
                clipRule="evenodd"
            />
        </svg>
    );
}
