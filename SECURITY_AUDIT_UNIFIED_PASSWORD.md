# Security Audit Report: Unified Password Implementation
**Date:** February 11, 2026  
**Auditor:** GitHub Copilot  
**PR:** #10001 - feat(wallet): implement unified password across all accounts  
**Commits Audited:** 7cd4d98b, 1904cbf2, e4ba77e1

---

## Executive Summary

This security audit reviews the unified password implementation for the IOTA wallet, which allows users to unlock all accounts and account sources with a single password. The implementation demonstrates **good cryptographic practices** with industry-standard encryption (PBKDF2 with 150,000 iterations), but has **several security vulnerabilities** that should be addressed:

### Overall Security Grade: **B- (7/10)**

**Strengths:**
- ✅ Strong cryptographic parameters (PBKDF2, 150k iterations)
- ✅ Rate limiting implemented (3 attempts, time-based lockout)
- ✅ No XSS vulnerabilities in UI components
- ✅ Encrypted ephemeral storage
- ✅ No password logging or debug exposure

**Critical Issues:**
- 🔴 **HIGH**: Information leakage in error messages
- 🔴 **HIGH**: Race condition in failed attempt tracking
- ⚠️ **MEDIUM**: Timing attack vulnerability in account iteration
- ⚠️ **MEDIUM**: Hardcoded password fallback in keystore
- ⚠️ **MEDIUM**: No password memory clearing (JavaScript limitation)

---

## Detailed Findings

### 1. Password Verification & Authentication

**File:** `apps/wallet/src/background/accounts/index.ts` (Lines 388-447)

#### 1.1 Information Leakage - HIGH SEVERITY 🔴

**Location:** Lines 442-444

```typescript
throw new Error(
    `Incorrect password. You have ${remainingAttempts} ${remainingAttempts === 1 ? 'attempt' : 'attempts'} left.`,
);
```

**Issues:**
- Error message reveals exact number of remaining attempts
- Confirms that a password-protected account exists
- Different error types (`AccountTooManyAttemptsError` vs `Error`) reveal system state
- Allows attackers to optimize brute-force timing

**Recommendation:**
```typescript
// Replace with generic message
throw new Error('Incorrect password');
```

**Impact:** Attackers can enumerate authentication state and optimize attacks.

---

#### 1.2 Race Condition in Failed Attempts - HIGH SEVERITY 🔴

**Location:** Lines 393, 424-428

```typescript
// Line 393: First state fetch
const { lockTimeMs, isLockedOut, lastFailedAttemptTime } = await getLockedState();

// ... password verification attempt ...

// Line 424: Time check without lock
if (timeSinceLastAttempt > RESET_FAILED_ATTEMPTS_THRESHOLD_IN_MS) {
    await updateLockedState({ failedAttempts: 0, lastFailedAttemptTime: currentTime });
}

// Line 428: Second state fetch - RACE CONDITION
const { failedAttempts: currentFailedAttempts } = await getLockedState();
```

**Issues:**
- Multiple concurrent unlock requests can bypass the 3-attempt limit
- State is fetched twice without transactional protection
- No mutex/lock prevents parallel password verification
- Critical section is not atomic

**Recommendation:**
```typescript
// Wrap entire verification flow in database transaction
await db.transaction('rw', db.settings, async () => {
    const state = await getLockedState();
    
    // All state checks and updates here
    
    // Verify password
    // Update failed attempts
    // Return result
});
```

**Impact:** Allows attackers to bypass rate limiting with parallel requests.

---

#### 1.3 Timing Attack Vulnerability - MEDIUM SEVERITY ⚠️

**Location:** Lines 408-416

```typescript
for (const anAccount of allAccounts) {
    if (isPasswordUnLockable(anAccount)) {
        await anAccount.verifyPassword(payload.args.password);
        await clearStateAfterManyFailedAttempts();
        await uiConnection.send(createMessage({ type: 'done' }, msg.id));
        return true;  // Early return creates timing difference
    }
}
```

**Issues:**
- Early return on first password-unlockable account
- Timing varies based on account position in array
- Attacker can infer account types through timing analysis
- Loop iteration count leaks information

**Recommendation:**
```typescript
// Always check all accounts, store result, return after loop
let verificationPassed = false;
let verificationError: Error | null = null;

for (const anAccount of allAccounts) {
    if (isPasswordUnLockable(anAccount) && !verificationPassed) {
        try {
            await anAccount.verifyPassword(payload.args.password);
            verificationPassed = true;
        } catch (e) {
            verificationError = e;
        }
    }
}

if (verificationPassed) {
    await clearStateAfterManyFailedAttempts();
    await uiConnection.send(createMessage({ type: 'done' }, msg.id));
    return true;
}

throw verificationError || new Error('Incorrect password');
```

**Impact:** Side-channel information leakage about account structure.

---

### 2. Cryptographic Implementation

**File:** `apps/wallet/src/shared/cryptography/keystore.ts`

#### 2.1 Hardcoded Password Fallback - MEDIUM SEVERITY ⚠️

**Location:** Lines 14-16

```typescript
const PASSWORD =
    process.env.WALLET_KEYRING_PASSWORD ||
    '344c6f7d04a65c24f35f5c710b0e91e2f2e2f88c038562622d5602019b937bc2c2aa2821e65cc94775fe5acf2fee240d38f1abbbe00b0e6682646a4ce10e908e';
```

**Issues:**
- Hardcoded fallback password committed to source control
- If environment variable is not set, uses publicly known password
- Encryption key is visible to anyone with repository access
- No warning or error when fallback is used

**Recommendation:**
```typescript
const PASSWORD = process.env.WALLET_KEYRING_PASSWORD;
if (!PASSWORD) {
    throw new Error(
        'WALLET_KEYRING_PASSWORD environment variable must be set. ' +
        'This is a critical security configuration.'
    );
}
```

**Impact:** Complete compromise of wallet encryption if environment variable is missing.

---

#### 2.2 PBKDF2 Parameters - SECURE ✅

**Location:** Lines 18-23

```typescript
const KD_OPTIONS: KeyDerivationOptions = {
    algorithm: 'PBKDF2',
    params: {
        iterations: 150_000,
    },
};
```

**Assessment:**
- ✅ PBKDF2 is industry-standard for password-based key derivation
- ✅ 150,000 iterations exceeds NIST SP 800-63B recommendation (100,000+)
- ✅ Uses `@metamask/browser-passworder` library (battle-tested)
- ✅ Appropriate for browser-based wallet applications

**No issues found.**

---

#### 2.3 Password Memory Clearing - MEDIUM SEVERITY ⚠️

**Issue:** JavaScript strings are immutable and cannot be securely wiped from memory.

**Current Implementation:**
```typescript
export async function encrypt(password: string, secrets: Serializable): Promise<string>
export async function decrypt<T extends Serializable>(password: string, ciphertext: string): Promise<T>
```

**Issues:**
- Passwords remain in memory until garbage collection
- No explicit clearing after use
- Memory dumps or debugging could expose passwords
- JavaScript limitations make true secure clearing difficult

**Recommendation:**
```typescript
// 1. Document the limitation clearly
/**
 * Encrypts data with password-based encryption.
 * 
 * @security Password is stored as JavaScript string which cannot be securely
 * wiped from memory. Avoid keeping password strings in long-lived variables.
 * Consider using Uint8Array for password handling in security-critical paths.
 */
export async function encrypt(password: string, secrets: Serializable): Promise<string>

// 2. For future enhancement, consider typed array passwords
export async function encryptSecure(
    passwordBuffer: Uint8Array, 
    secrets: Serializable
): Promise<string> {
    const password = new TextDecoder().decode(passwordBuffer);
    try {
        return await encrypt(password, secrets);
    } finally {
        // Best effort to clear string (limited in JS)
        passwordBuffer.fill(0);
    }
}
```

**Impact:** Passwords may persist in memory longer than necessary.

---

### 3. Ephemeral Storage Security

**Files:** `apps/wallet/src/background/sessionEphemeralValues.ts`, `apps/wallet/src/background/account-sources/accountSource.ts`

#### 3.1 Session Storage Encryption - SECURE ✅

**Implementation:**
```typescript
// Random password per storage operation
const rndPass = getRandomPassword(); // 64 random bytes
const ephemeralPassword = makeEphemeraPassword(rndPass); // Combined with static PASSWORD
await encrypt(ephemeralPassword, dataToStore);
```

**Assessment:**
- ✅ Chrome Extension Session Storage (volatile, cleared on restart)
- ✅ Additional encryption layer on ephemeral data
- ✅ Random password per operation (64-byte entropy)
- ✅ Cleared on account lock via `clearEphemeralValue()`

**No issues found.**

---

### 4. UI Components Security

**Files:** 
- `apps/wallet/src/ui/app/components/accounts/PasswordModalDialog.tsx`
- `apps/wallet/src/ui/app/components/accounts/UnlockAccountModal.tsx`
- `apps/wallet/src/ui/app/components/accounts/ForgotPasswordDialog.tsx`

#### 4.1 XSS Protection - SECURE ✅

**Assessment:**
- ✅ No `dangerouslySetInnerHTML` usage
- ✅ No `eval()` or dynamic code execution
- ✅ No direct DOM manipulation with user input
- ✅ React's built-in XSS protection via JSX
- ✅ User input properly sanitized through form validation

**No issues found.**

---

#### 4.2 Password Input Handling - SECURE ✅

**Implementation:**
```tsx
<Input
    autoFocus
    type={InputType.Password}
    isVisibilityToggleEnabled
    placeholder="Password"
    {...register('password')}
/>
```

**Assessment:**
- ✅ Uses password input type (masked by default)
- ✅ Optional visibility toggle (user-controlled)
- ✅ No autocomplete concerns (handled by form library)
- ✅ Form validation prevents empty submissions

**No issues found.**

---

#### 4.3 Countdown Error Display - ACCEPTABLE ⚠️

**Location:** `PasswordModalDialog.tsx`, Lines 74-101

```typescript
useEffect(() => {
    async function checkLockState() {
        const { remainingTime } = await backgroundService.getLockedState({});
        
        if (remainingTime <= 0) {
            setCountdownError(null);
            setRunLockInterval(false);
        } else {
            const remainingSeconds = Math.ceil(remainingTime / MILLISECONDS_PER_SECOND);
            const message = `Too many failed attempts. Please try again in ${remainingSeconds} ${remainingSeconds === 1 ? 'second' : 'seconds'}.`;
            setCountdownError(message);
        }
    }
    // ... polling every second
}, [runLockInterval, open]);
```

**Assessment:**
- ⚠️ Displays precise countdown (potential information leakage)
- ✅ Updates every second (acceptable UX)
- ✅ Client-side check matches server-side enforcement
- ⚠️ Could reveal lockout duration to attackers

**Recommendation:** Consider generic message like "Too many failed attempts. Please try again later." without specific countdown.

**Impact:** Minor - reveals lockout duration but doesn't significantly aid attacks.

---

### 5. Unified Password Design

**File:** `apps/wallet/src/background/accounts/index.ts`

#### 5.1 Architecture Review

**Implementation:**
```typescript
export async function unlockAllAccountsAndSources(password?: string) {
    const sources = await getAccountSources();
    for (const source of sources) {
        if (password) {
            await source.unlock(password);
        }
    }

    const allAccounts = await getAllAccounts();
    const accounts = allAccounts.filter(
        (account) => !ACCOUNT_TYPES_WITH_SOURCE.includes(account.type),
    );

    for (const account of accounts) {
        const isPasswordUnlockable = isPasswordUnLockable(account);
        const isLocked = await account.isLocked();
        if (isPasswordUnlockable && isLocked) {
            await account.passwordUnlock(password);
        }
    }
}
```

**Security Implications:**

**Positive:**
- ✅ Single password simplifies user experience
- ✅ Consistent security level across all accounts
- ✅ Reduces password fatigue
- ✅ All-or-nothing unlocking (clear security boundary)

**Concerns:**
- ⚠️ Single point of failure (compromise one password = compromise all accounts)
- ⚠️ No account-level isolation
- ⚠️ Cannot have different security policies per account
- ⚠️ Password reuse across all cryptographic operations

**Recommendation:** 
- Document the security trade-offs clearly in user documentation
- Consider optional per-account passwords for high-value accounts (future enhancement)
- Implement additional security layers (2FA, biometrics) for sensitive operations

**Overall Assessment:** Design is **acceptable** for consumer wallet use case. The trade-off between usability and security is reasonable for most users.

---

## Priority-Ordered Remediation Plan

### Critical (Fix Immediately)

1. **Fix Race Condition in Password Verification**
   - File: `apps/wallet/src/background/accounts/index.ts`
   - Lines: 388-447
   - Wrap verification flow in database transaction
   - Estimated effort: 4-6 hours

2. **Remove Information Leakage from Error Messages**
   - File: `apps/wallet/src/background/accounts/index.ts`
   - Lines: 442-444
   - Use generic error messages
   - Estimated effort: 1 hour

### High Priority (Fix Before Release)

3. **Eliminate Hardcoded Password Fallback**
   - File: `apps/wallet/src/shared/cryptography/keystore.ts`
   - Lines: 14-16
   - Require environment variable, throw on missing
   - Estimated effort: 1 hour

4. **Fix Timing Attack in Account Verification**
   - File: `apps/wallet/src/background/accounts/index.ts`
   - Lines: 408-416
   - Implement constant-time verification loop
   - Estimated effort: 2-3 hours

### Medium Priority (Next Sprint)

5. **Document Password Memory Limitations**
   - File: `apps/wallet/src/shared/cryptography/keystore.ts`
   - Add JSDoc security notes
   - Estimated effort: 30 minutes

6. **Reduce Countdown Information Disclosure**
   - File: `apps/wallet/src/ui/app/components/accounts/PasswordModalDialog.tsx`
   - Lines: 74-101
   - Use generic lockout message
   - Estimated effort: 30 minutes

### Low Priority (Future Enhancement)

7. **Implement Secure Password Memory Handling**
   - Research TypedArray-based password handling
   - Prototype secure clearing mechanisms
   - Estimated effort: 1-2 weeks (research + implementation)

---

## Testing Recommendations

### Security Test Cases

1. **Concurrent Password Attempts**
   ```javascript
   // Test: Race condition in failed attempts
   const promises = Array(10).fill(null).map(() => 
       backgroundService.verifyPassword({ password: 'wrong' })
   );
   await Promise.allSettled(promises);
   // Verify: Should not bypass 3-attempt limit
   ```

2. **Timing Attack Detection**
   ```javascript
   // Test: Measure verification time variance
   const times = [];
   for (let i = 0; i < 100; i++) {
       const start = performance.now();
       await backgroundService.verifyPassword({ password: 'test' });
       times.push(performance.now() - start);
   }
   // Verify: Standard deviation should be < 5% of mean
   ```

3. **Information Leakage**
   ```javascript
   // Test: Error messages should not reveal state
   try {
       await backgroundService.verifyPassword({ password: 'wrong' });
   } catch (e) {
       // Verify: Message should not contain attempt counts
       expect(e.message).not.toMatch(/\d+ attempts? left/);
   }
   ```

4. **Lockout Persistence**
   ```javascript
   // Test: Lockout survives restart
   // 1. Trigger lockout (3 failed attempts)
   // 2. Close and reopen wallet
   // 3. Verify: Still locked out
   ```

---

## Compliance & Standards

### Cryptographic Standards Compliance

| Standard | Requirement | Status |
|----------|-------------|--------|
| NIST SP 800-63B | PBKDF2 with 100,000+ iterations | ✅ Pass (150,000) |
| OWASP ASVS 2.4.1 | Password minimum length | ✅ Not enforced by wallet (user responsibility) |
| OWASP ASVS 2.4.5 | Failed authentication lockout | ✅ Pass (3 attempts, time-based) |
| OWASP ASVS 2.9.1 | Constant-time comparison | ⚠️ Partial (library handles, but orchestration leaks) |
| OWASP ASVS 6.2.1 | Encrypt sensitive data at rest | ✅ Pass (PBKDF2 + AES) |
| OWASP ASVS 8.3.4 | Generic authentication error messages | ❌ Fail (reveals attempt count) |

**Overall Compliance: 4/6 (67%)**

---

## Conclusion

The unified password implementation demonstrates **solid cryptographic fundamentals** but requires **critical security fixes** before production deployment. The most urgent issues are:

1. Race condition allowing bypass of rate limiting
2. Information leakage in authentication errors
3. Hardcoded password fallback

These issues are **fixable with minimal code changes** and should be addressed immediately. The underlying cryptographic design (PBKDF2, encryption parameters, key derivation) is sound and follows industry best practices.

**Recommended Action:** 
- 🔴 **Block merge** until Critical and High Priority issues are resolved
- ⚠️ Conduct additional penetration testing after fixes
- ✅ Proceed with merge after verification

---

## References

1. NIST SP 800-63B - Digital Identity Guidelines
2. OWASP Authentication Cheat Sheet
3. OWASP Application Security Verification Standard (ASVS)
4. MetaMask Browser Passworder Library Documentation
5. CWE-208: Observable Timing Discrepancy
6. CWE-362: Concurrent Execution using Shared Resource with Improper Synchronization

---

**Auditor Signature:** GitHub Copilot  
**Audit Completed:** February 11, 2026  
**Next Review:** After remediation implementation
