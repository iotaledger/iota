# Required Security Fixes for Unified Password PR

## Critical Issues (Must Fix Before Merge)

### 1. Race Condition in Password Verification ⚠️ HIGH RISK

**File:** `apps/wallet/src/background/accounts/index.ts`  
**Lines:** 388-447  
**Issue:** Multiple concurrent unlock attempts can bypass the 3-attempt limit

**Fix:**
```typescript
if (isMethodPayload(payload, 'verifyPassword')) {
    const MAX_UNLOCK_ATTEMPTS = 3;
    const RESET_FAILED_ATTEMPTS_THRESHOLD_IN_MS = 60 * SECONDS_PER_MINUTE * MILLISECONDS_PER_SECOND;

    // Wrap entire operation in a transaction to prevent race conditions
    const db = await getDB();
    await db.transaction('rw', db.settings, db.accounts, async () => {
        const { lockTimeMs, isLockedOut, lastFailedAttemptTime } = await getLockedState();

        if (isLockedOut && lockTimeMs) {
            const elapsedTime = Date.now() - Number(lockTimeMs);
            const remainingTime = Math.max(0, WALLET_LOCK_DURATION_IN_MS - elapsedTime);

            if (remainingTime > 0) {
                throw new AccountTooManyAttemptsError();
            } else {
                await clearStateAfterManyFailedAttempts();
            }
        }

        try {
            const allAccounts = await getAllAccounts();
            let verificationPassed = false;
            
            for (const anAccount of allAccounts) {
                if (isPasswordUnLockable(anAccount) && !verificationPassed) {
                    try {
                        await anAccount.verifyPassword(payload.args.password);
                        verificationPassed = true;
                    } catch (e) {
                        // Continue to check all accounts for constant-time behavior
                    }
                }
            }
            
            if (verificationPassed) {
                await clearStateAfterManyFailedAttempts();
                await uiConnection.send(createMessage({ type: 'done' }, msg.id));
                return true;
            }
            
            throw new Error('Incorrect password');
        } catch (error) {
            const currentTime = Date.now();
            const lastFailedAttempt = lastFailedAttemptTime || 0;
            const timeSinceLastAttempt = currentTime - Number(lastFailedAttempt);

            if (timeSinceLastAttempt > RESET_FAILED_ATTEMPTS_THRESHOLD_IN_MS) {
                await updateLockedState({ failedAttempts: 0, lastFailedAttemptTime: currentTime });
            }

            const { failedAttempts: currentFailedAttempts } = await getLockedState();
            const failedAttempts = Number(currentFailedAttempts) + 1;

            if (failedAttempts >= MAX_UNLOCK_ATTEMPTS) {
                await updateLockedState({
                    lockTimeMs: Date.now(),
                    isLockedOut: true,
                });
                throw new AccountTooManyAttemptsError();
            } else {
                await updateLockedState({ failedAttempts, lastFailedAttemptTime: currentTime });
                throw new Error('Incorrect password');
            }
        }
    });
}
```

---

### 2. Information Leakage in Error Messages ⚠️ HIGH RISK

**File:** `apps/wallet/src/background/accounts/index.ts`  
**Lines:** 442-444  
**Issue:** Error reveals exact number of remaining attempts

**Current Code:**
```typescript
throw new Error(
    `Incorrect password. You have ${remainingAttempts} ${remainingAttempts === 1 ? 'attempt' : 'attempts'} left.`,
);
```

**Fix:**
```typescript
// Replace with generic message
throw new Error('Incorrect password');
```

**Also update UI to not display attempt counts:**
```typescript
// In PasswordModalDialog.tsx - already handles AccountTooManyAttemptsError correctly
// Just ensure no other places display attempt counts
```

---

### 3. Remove Hardcoded Password Fallback ⚠️ HIGH RISK

**File:** `apps/wallet/src/shared/cryptography/keystore.ts`  
**Lines:** 14-16  
**Issue:** Fallback to publicly known password

**Current Code:**
```typescript
const PASSWORD =
    process.env.WALLET_KEYRING_PASSWORD ||
    '344c6f7d04a65c24f35f5c710b0e91e2f2e2f88c038562622d5602019b937bc2...';
```

**Fix:**
```typescript
const PASSWORD = process.env.WALLET_KEYRING_PASSWORD;
if (!PASSWORD) {
    throw new Error(
        'FATAL: WALLET_KEYRING_PASSWORD environment variable is not set. ' +
        'This is required for wallet encryption. Please configure it before starting the application.'
    );
}
```

**Update build/deployment documentation** to require setting this environment variable.

---

## Medium Priority Issues (Fix Soon)

### 4. Timing Attack in Account Verification

**File:** `apps/wallet/src/background/accounts/index.ts`  
**Lines:** 408-416  
**Issue:** Early return creates timing difference

This is already addressed in Fix #1 above (constant-time loop implementation).

---

### 5. Reduce Information Disclosure in UI

**File:** `apps/wallet/src/ui/app/components/accounts/PasswordModalDialog.tsx`  
**Lines:** 87  
**Issue:** Shows precise countdown

**Current Code:**
```typescript
const message = `Too many failed attempts. Please try again in ${remainingSeconds} ${remainingSeconds === 1 ? 'second' : 'seconds'}.`;
```

**Fix:**
```typescript
const message = `Too many failed attempts. Please try again later.`;
```

Or if countdown is desired for UX, it's acceptable to keep as-is (minor information leakage, not critical).

---

## Documentation Improvements

### 6. Document Password Memory Limitations

**File:** `apps/wallet/src/shared/cryptography/keystore.ts`  
**Lines:** 34-43  

**Add JSDoc:**
```typescript
/**
 * Encrypts data using password-based encryption.
 * 
 * Uses PBKDF2 with 150,000 iterations for key derivation.
 * 
 * @security WARNING: Password is stored as JavaScript string which cannot be
 * securely wiped from memory due to string immutability. Passwords may persist
 * in memory until garbage collection. Avoid storing passwords in long-lived
 * variables. For enhanced security in future, consider TypedArray-based password
 * handling.
 * 
 * @param password - User password for encryption
 * @param secrets - Data to encrypt (must be JSON-serializable)
 * @returns Encrypted ciphertext as string
 */
export async function encrypt(password: string, secrets: Serializable): Promise<string> {
    return metamaskEncrypt(password, secrets, undefined, undefined, KD_OPTIONS);
}

/**
 * Decrypts password-encrypted data.
 * 
 * @security Same password memory limitations as encrypt().
 * 
 * @param password - User password for decryption
 * @param ciphertext - Encrypted data to decrypt
 * @returns Decrypted data
 * @throws {Error} If password is incorrect or ciphertext is invalid
 */
export async function decrypt<T extends Serializable>(
    password: string,
    ciphertext: string,
): Promise<T> {
    return (await metamaskDecrypt(password, ciphertext)) as T;
}
```

---

## Testing Requirements

Add these test cases to verify fixes:

### Test 1: Race Condition Prevention
```typescript
test('should not bypass rate limiting with concurrent requests', async () => {
    // Attempt 10 concurrent wrong password verifications
    const promises = Array(10).fill(null).map(() => 
        backgroundClient.verifyPassword({ password: 'wrongpassword' })
            .catch(e => e)
    );
    
    const results = await Promise.allSettled(promises);
    const errors = results.map(r => r.status === 'fulfilled' ? r.value : r.reason);
    
    // Should have exactly 3 incorrect password errors, then lockout
    const incorrectPasswordErrors = errors.filter(e => 
        e.message === 'Incorrect password'
    );
    const lockoutErrors = errors.filter(e => 
        AccountTooManyAttemptsError.is(e)
    );
    
    expect(incorrectPasswordErrors.length).toBeLessThanOrEqual(3);
    expect(lockoutErrors.length).toBeGreaterThan(0);
});
```

### Test 2: No Information Leakage
```typescript
test('should not reveal attempt count in errors', async () => {
    for (let i = 0; i < 3; i++) {
        try {
            await backgroundClient.verifyPassword({ password: 'wrong' });
        } catch (e) {
            expect(e.message).toBe('Incorrect password');
            expect(e.message).not.toMatch(/\d+/); // No numbers in message
            expect(e.message).not.toMatch(/attempt/i);
        }
    }
});
```

### Test 3: Environment Variable Required
```typescript
test('should throw if WALLET_KEYRING_PASSWORD not set', () => {
    delete process.env.WALLET_KEYRING_PASSWORD;
    
    // Re-import module to trigger initialization
    jest.resetModules();
    
    expect(() => {
        require('../shared/cryptography/keystore');
    }).toThrow(/WALLET_KEYRING_PASSWORD/);
});
```

---

## Build/Deployment Changes

### Update `.env.example`:
```bash
# Required: Master password for wallet encryption
# Generate with: openssl rand -hex 64
WALLET_KEYRING_PASSWORD=your_randomly_generated_256_bit_hex_password_here
```

### Update `README.md`:
```markdown
## Environment Variables

The following environment variables are **required** for security:

- `WALLET_KEYRING_PASSWORD`: Master encryption key for wallet storage. 
  - **Must** be set before starting the application
  - Generate with: `openssl rand -hex 64`
  - Store securely (e.g., environment secrets, not in source code)
  - Different value per environment (dev, staging, prod)
```

---

## Estimated Effort

| Fix | Time | Priority |
|-----|------|----------|
| Fix #1: Race condition | 4-6 hours | Critical |
| Fix #2: Error messages | 1 hour | Critical |
| Fix #3: Hardcoded password | 1 hour | Critical |
| Fix #5: UI countdown | 30 min | Medium |
| Fix #6: Documentation | 30 min | Medium |
| Testing | 4 hours | Critical |
| **Total** | **11-13 hours** | |

---

## Verification Checklist

Before merging:
- [ ] All Critical fixes implemented
- [ ] Unit tests added and passing
- [ ] Integration tests verify rate limiting works
- [ ] No hardcoded passwords in codebase
- [ ] Environment variable documentation updated
- [ ] Security audit re-run and passed
- [ ] Code review by security team
- [ ] Manual penetration testing completed

---

## Additional Security Recommendations (Future Enhancements)

1. **Add Security Event Logging**
   - Log all authentication failures
   - Log lockout events
   - Log password changes
   - Useful for security monitoring and forensics

2. **Consider Additional Authentication Factors**
   - Biometric authentication (where available)
   - Hardware security keys (WebAuthn)
   - Time-based OTP for sensitive operations

3. **Implement Memory-Safe Password Handling**
   - Research TypedArray-based password APIs
   - Investigate libsodium.js for secure memory operations
   - Consider Rust/WebAssembly for security-critical crypto operations

4. **Add Account-Level Security Options**
   - Optional per-account passwords for high-value accounts
   - Different security levels (convenience vs. paranoid)
   - Transaction amount thresholds requiring re-authentication

5. **Security Monitoring Dashboard**
   - Display recent authentication attempts
   - Show active sessions
   - Alert on suspicious activity

---

**Document Version:** 1.0  
**Last Updated:** February 11, 2026  
**Status:** AWAITING IMPLEMENTATION
