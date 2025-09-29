import { BrowserContext, Page, expect } from '@playwright/test';

/**
 * Set up receiver address in bridge UI
 */
export async function setReceiverAddress(page: Page, address: string): Promise<void> {
    const toggleManualInput = page.getByTestId('toggle-receiver-address-input');
    await expect(toggleManualInput).toBeVisible();
    await toggleManualInput.click();

    const addressField = page.getByTestId('receive-address');
    await expect(addressField).toBeVisible();
    await addressField.fill(address);
}

/**
 * Toggle bridge direction from L1→L2 to L2→L1
 */
export async function toggleBridgeDirection(page: Page): Promise<void> {
    const toggleButton = page.getByTestId('toggle-bridge-direction');
    await expect(toggleButton).toBeVisible({ timeout: 5000 });
    await toggleButton.click();
}

/**
 * Select a coin in the bridge UI
 */
export async function selectCoin(page: Page, coinName: string): Promise<void> {
    await page.getByTestId('coin-selector').click();
    await page.getByText(coinName, { exact: true }).first().click();
}

/**
 * Set bridge amount
 */
export async function setBridgeAmount(page: Page, amount: string | number): Promise<void> {
    const amountField = page.getByTestId('bridge-amount');
    await expect(amountField).toBeVisible();
    await amountField.fill(amount.toString());
}

/**
 * Click max amount button
 */
export async function clickMaxAmount(page: Page): Promise<void> {
    await page.getByText('Max').click();
}

/**
 * Execute bridge transaction and approve it
 */
export async function executeBridgeTransaction(
    page: Page,
    browserContext: BrowserContext,
    isL1: boolean,
): Promise<void> {
    await expect(page.getByText('Bridge Assets')).toBeEnabled();

    const approvePagePromise = browserContext.waitForEvent('page');
    await page.getByText('Bridge Assets').click();

    const approvePage = await approvePagePromise;
    await approvePage.waitForLoadState();

    const buttonName = isL1 ? 'Approve' : 'Confirm';
    await approvePage.getByRole('button', { name: buttonName }).click();
}
