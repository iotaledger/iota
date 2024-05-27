// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { usePopup } from '@/hooks';
import styles from './Popup.module.css';
import { PopupManager } from '@/lib/interfaces';

const Popup: React.FC = () => {
    const { popups, closePopup } = usePopup() as PopupManager;

    if (popups.length === 0) return null;

    return (
        <>
            {popups.map((popup, index) => (
                <div key={index} className={styles.overlay}>
                    <div className={styles.popupWrapper}>
                        <div className={styles.popupContainer}>
                            <div className={styles.popup}>
                                <button className={styles.closeButton} onClick={closePopup}>
                                    X
                                </button>
                                {popup}
                            </div>
                        </div>
                    </div>
                </div>
            ))}
        </>
    );
};

export default Popup;
