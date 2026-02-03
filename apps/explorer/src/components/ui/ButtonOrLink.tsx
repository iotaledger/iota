// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LinkWithQuery, type RouterLinkProps } from './LinkWithQuery';

export interface ButtonOrLinkProps {
    href?: string;
    to?: RouterLinkProps['to'];
    ref?: React.Ref<HTMLAnchorElement | HTMLButtonElement>;
    [key: string]: any;
}

export function ButtonOrLink({ href, to, ref, ...props }: ButtonOrLinkProps): React.JSX.Element {
    // External link:
    if (href) {
        return (
            // eslint-disable-next-line jsx-a11y/anchor-has-content
            <a
                ref={ref as React.Ref<HTMLAnchorElement>}
                target="_blank"
                rel="noreferrer noopener"
                href={href}
                {...props}
            />
        );
    }

    // Internal router link:
    if (to) {
        return <LinkWithQuery to={to} ref={ref as React.Ref<HTMLAnchorElement>} {...props} />;
    }

    // We set the default type to be "button" to avoid accidentally submitting forms.
    // eslint-disable-next-line react/button-has-type
    return (
        <button
            {...props}
            type={props.type || 'button'}
            ref={ref as React.Ref<HTMLButtonElement>}
        />
    );
}

ButtonOrLink.displayName = 'ButtonOrLink';
