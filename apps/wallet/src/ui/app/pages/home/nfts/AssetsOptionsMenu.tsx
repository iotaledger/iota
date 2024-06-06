// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Menu } from '@headlessui/react';
import { Ooo24 } from '@iota/icons';
import { Link } from 'react-router-dom';

const AssetsOptionsMenu = () => {
    return (
        <Menu>
            <Menu.Button
                style={{
                    border: 'none',
                    background: 'none',
                    height: '32px',
                    width: '32px',
                    cursor: 'pointer',
                }}
            >
                <Ooo24 className="h-full w-full text-gray-90" />
            </Menu.Button>
            <Menu.Items className="divide-gray-200 absolute right-0 top-4 z-50 mt-2 w-50 divide-y rounded-md bg-white">
                <div className="h-full w-full rounded-md p-2 shadow-card-soft">
                    <Menu.Item>
                        {({ active }) => (
                            <Link
                                to="/nfts/hidden-assets"
                                className="text-bodySmall font-medium text-steel-darker no-underline hover:text-steel-darker focus:text-steel-darker disabled:text-steel-darker"
                            >
                                <div className="rounded-md bg-opacity-50 p-3 hover:bg-iota-light">
                                    View Hidden Assets
                                </div>
                            </Link>
                        )}
                    </Menu.Item>
                </div>
            </Menu.Items>
        </Menu>
    );
};

export default AssetsOptionsMenu;
