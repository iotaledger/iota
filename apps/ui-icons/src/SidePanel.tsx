// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgSidePanel(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            {...props}
        >
            <path
                fill="currentColor"
                d="M10.8333 9.99984L6.66663 13.3332V6.6665L10.8333 9.99984Z"
            />
            <path
                fill="currentColor"
                fill-rule="evenodd"
                clip-rule="evenodd"
                d="M16.3452 2.49984C17.4482 2.49984 18.3453 3.3964 18.3455 4.49935L18.3333 15.5443C18.3332 16.6473 17.436 17.4998 16.333 17.4998L3.66695 17.4551C2.56387 17.4551 1.66669 16.6026 1.66663 15.4995L1.67883 4.4554C1.67883 3.35236 2.57533 2.45521 3.67834 2.45508L16.3452 2.49984ZM3.33329 15.8324H12.5V4.1665H3.33329V15.8324ZM14.1666 15.8324H16.6666V4.1665H14.1666V15.8324Z"
            />
        </svg>
    );
}
