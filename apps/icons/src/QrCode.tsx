// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
const SvgQrCode = (props: SVGProps<SVGSVGElement>) => (
    <svg
        xmlns="http://www.w3.org/2000/svg"
        width="1em"
        height="1em"
        fill="none"
        viewBox="0 0 24 24"
        {...props}
    >
        <path
            stroke="#C3C5C8"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M7.111 7.111h.009m9.769 0h.009M7.11 16.89h.009m5.769-4h.009m3.99 4h.01M16.444 20H20v-3.556M13.778 16v4M20 13.778h-4m-.8-3.556h3.378c.498 0 .747 0 .937-.097a.889.889 0 0 0 .388-.388C20 9.547 20 9.297 20 8.8V5.422c0-.498 0-.747-.097-.937a.889.889 0 0 0-.388-.388C19.325 4 19.075 4 18.578 4H15.2c-.498 0-.747 0-.937.097a.889.889 0 0 0-.388.388c-.097.19-.097.44-.097.937V8.8c0 .498 0 .747.097.937a.889.889 0 0 0 .388.388c.19.097.44.097.937.097Zm-9.778 0H8.8c.498 0 .747 0 .937-.097a.888.888 0 0 0 .388-.388c.097-.19.097-.44.097-.937V5.422c0-.498 0-.747-.097-.937a.889.889 0 0 0-.388-.388C9.547 4 9.297 4 8.8 4H5.422c-.498 0-.747 0-.937.097a.889.889 0 0 0-.388.388C4 4.675 4 4.925 4 5.422V8.8c0 .498 0 .747.097.937a.889.889 0 0 0 .388.388c.19.097.44.097.937.097Zm0 9.778H8.8c.498 0 .747 0 .937-.097a.889.889 0 0 0 .388-.388c.097-.19.097-.44.097-.937V15.2c0-.498 0-.747-.097-.937a.889.889 0 0 0-.388-.388c-.19-.097-.44-.097-.937-.097H5.422c-.498 0-.747 0-.937.097a.889.889 0 0 0-.388.388C4 14.453 4 14.703 4 15.2v3.378c0 .498 0 .747.097.937a.889.889 0 0 0 .388.388c.19.097.44.097.937.097Z"
        />
    </svg>
);
export default SvgQrCode;
