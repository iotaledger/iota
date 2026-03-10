// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import Link from 'next/link';
import { createPortal } from 'react-dom';

import type { ExternalRoute, Route } from '@/lib/interfaces';

type NavbarMobileContainerProps = {
    logo: React.ReactNode;
    routes: Route[];
    onClose: () => void;
    isOpen: boolean;
};

const isExternalRoute = (r: Route): r is ExternalRoute =>
    'isExternal' in r && r.isExternal === true;

export function NavbarMobileContainer({
    logo,
    routes,
    children,
    onClose,
    isOpen,
}: React.PropsWithChildren<NavbarMobileContainerProps>) {
    return createPortal(
        <div
            className={clsx(
                'fixed inset-0 z-[60] backdrop-blur-lg transition-opacity duration-300',
                isOpen ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none',
            )}
            onMouseDown={onClose}
            role="dialog"
            aria-modal="true"
            aria-hidden={!isOpen}
        >
            <div
                onMouseDown={(e) => e.stopPropagation()}
                className={clsx(
                    'absolute top-0 left-0 right-0',
                    'transition-transform duration-300 ease-out',
                    isOpen ? 'translate-y-0' : '-translate-y-full',
                )}
            >
                <div className="px-lg py-md flex flex-row items-center justify-between bg-names-neutral-6">
                    {logo}
                    <button
                        type="button"
                        className="w-6 h-6 [&_svg]:w-full [&_svg]:h-full hover:text-names-primary-80"
                        onClick={onClose}
                        aria-label="Close menu"
                    >
                        <Close />
                    </button>
                </div>

                <div className="px-lg pb-xl bg-names-neutral-6">
                    <nav className="flex flex-col gap-y-4">
                        {routes.map((route) => (
                            <Link
                                key={route.id ?? route.path}
                                href={route.path}
                                className="text-label-lg hover:text-names-primary-80 ml-xxs"
                                {...(isExternalRoute(route)
                                    ? { target: '_blank', rel: 'noopener noreferrer' }
                                    : {})}
                                onClick={onClose}
                            >
                                {route.title}
                            </Link>
                        ))}
                        {children}
                    </nav>
                </div>
            </div>
        </div>,
        document.body,
    );
}
