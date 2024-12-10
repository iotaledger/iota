// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { usePopups } from '@/hooks';

function Popup(): JSX.Element {
    const { popups, closePopup } = usePopups();

    return (
        <>
            {popups.map((popup, index) => (
                <div
                    key={index}
                    className="fixed left-0 top-0 z-[1000] flex h-full w-full items-center justify-center bg-black/20"
                >
                    <div className="relative">
                        <div className="absolute left-2/4 top-2/4 flex max-h-[80vh] min-w-[200px] -translate-x-2/4 -translate-y-2/4 flex-col gap-3 overflow-y-auto rounded-lg bg-white p-5 text-black">
                            <button className="cursor-pointer self-end" onClick={closePopup}>
                                X
                            </button>
                            {popup}
                        </div>
                    </div>
                </div>
            ))}
        </>
    );
}

export default Popup;
