// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
// import { ButtonSize } from '@/lib/components';

import type { Meta, StoryObj } from '@storybook/react';
import { Activity, Apps, Assets, Home } from '@iota/ui-icons';
import { Navbar, NavbarSlideout, NavbarItemWithID, NavbarProps, NavbarType } from '@/components';
import { useState } from 'react';

const NAVBAR_ITEMS: NavbarItemWithID[] = [
    { id: 'home', icon: <Home /> },
    { id: 'assets', icon: <Assets /> },
    { id: 'apps', icon: <Apps /> },
    { id: 'activity', icon: <Activity /> },
];

const meta: Meta<NavbarProps> = {
    component: Navbar,
    tags: ['autodocs'],
    render: () => {
        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);

        return (
            <div className="flex w-1/3 border">
                <Navbar
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClick={(id) => setActiveId(id)}
                />
            </div>
        );
    },
} satisfies Meta<typeof Navbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Vertical: Story = {
    args: {},
    argTypes: {},
    render: (args) => {
        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);

        return (
            <div className="flex border border-gray-200">
                <Navbar
                    type={NavbarType.Vertical}
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClick={(id) => setActiveId(id)}
                />
            </div>
        );
    },
};
export const SlideOut: Story = {
    args: {},
    argTypes: {},
    render: (args) => {
        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);

        return (
            <div className="flex border border-gray-200">
                <NavbarSlideout
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClick={(id) => setActiveId(id)}
                />
            </div>
        );
    },
};
