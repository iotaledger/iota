// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

type PickedLinkProps = Pick<React.ComponentProps<'a'>, 'href' | 'children'>;

interface LinkProps extends PickedLinkProps {
    isExternal?: boolean;
    isSecondary?: boolean;
}

const LINK_STYLES = {
    primary:
        'text-iota-primary-30 dark:text-iota-primary-80 hover:text-iota-primary-50 dark:hover:text-iota-primary-60',
    secondary:
        'text-iota-neutral-40 dark:text-iota-neutral-60 hover:text-iota-neutral-60 dark:hover:text-iota-neutral-40',
};

const BASE_STYLES = 'transition-colors duration-150 text-body-md';

export function Link({
    isExternal,
    isSecondary,
    children,
    ...props
}: LinkProps): React.JSX.Element {
    const externalProps = isExternal ? { target: '_blank', rel: 'noopener noreferrer' } : {};
    const textStyles = isSecondary ? LINK_STYLES.secondary : LINK_STYLES.primary;
    return (
        <a {...externalProps} {...props} className={`${textStyles} ${BASE_STYLES}`}>
            {children}
        </a>
    );
}
