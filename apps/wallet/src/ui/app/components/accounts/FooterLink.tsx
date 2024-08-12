// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';

import { Link, type LinkProps } from '_app/shared/Link';

interface FooterLinkProps extends LinkProps {
    icon?: React.ReactNode;
}
const FooterLink = forwardRef((props: FooterLinkProps, forwardedRef) => {
    return (
        <div className="flex items-center justify-center gap-1 rounded-sm  bg-none p-1 uppercase hover:bg-white/60">
            <Link before={props.icon} weight="semibold" size="captionSmall" {...props} />
        </div>
    );
});

export { FooterLink };
