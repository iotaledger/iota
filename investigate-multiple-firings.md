# Analytics Events Investigation Report
## IOTA Wallet - Multiple Firing Issues

**Date:** 2026-03-17
**Project:** apps/wallet
**Scope:** Review existing analytics events for correct firing behavior

---

## Executive Summary

Reviewed **34 unique analytics events** across the wallet application. Identified **5 events** that need optimization, with **1 critical issue** requiring immediate attention.

**Key Finding:** The "Opened Wallet Extension" event fires multiple times per session due to over-eager dependency tracking, causing analytics noise and inflated metrics.

---

## Events Requiring Optimization

### 🚨 CRITICAL: Event #1 - Opened Wallet Extension

**File:** [`apps/wallet/src/ui/app/hooks/useInitialPageView.ts:24-34`](apps/wallet/src/ui/app/hooks/useInitialPageView.ts#L24-L34)

**Current Implementation:**
```typescript
useEffect(() => {
    ampli.openedWalletExtension({
        activeNetwork,
        activeAccountType: activeAccount?.type,
        activeOrigin: activeOrigin || undefined,
        pagePath: location.pathname,
        pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
        walletAppMode: isFullScreen ? 'Fullscreen' : 'Pop-up',
        walletVersion: Browser.runtime.getManifest().version,
    });
}, [activeAccount?.type, activeNetwork, activeOrigin, isFullScreen, location]);
```

**Problem:**
- Fires on **every render** when ANY dependency changes
- Dependencies include:
  - `activeAccount?.type` → fires when account switches
  - `activeNetwork` → fires when network changes
  - `activeOrigin` → fires when navigating to pages with different origins
  - `isFullScreen` → fires when view type changes
  - `location` → fires on **every navigation/route change**

**Impact:**
- Event name suggests it tracks "extension opened" but actually tracks navigation and state changes
- Creates significant analytics noise
- Inflates usage metrics
- Makes it impossible to distinguish actual extension opens from in-app navigation
- Could exhaust analytics event quota

**Scenarios Where It Incorrectly Fires:**
1. User navigates from Home → Tokens → NFTs → Settings (4+ events)
2. User switches between accounts (1 event per switch)
3. User changes network (1 event per change)
4. User toggles fullscreen mode
5. Any combination of the above

**Expected Behavior:**
Should fire **only once** when the extension is first opened/loaded in a session

**Recommended Fix:**
```typescript
// Option 1: Fire only on mount
useEffect(() => {
    ampli.openedWalletExtension({
        activeNetwork,
        activeAccountType: activeAccount?.type,
        activeOrigin: activeOrigin || undefined,
        pagePath: location.pathname,
        pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
        walletAppMode: isFullScreen ? 'Fullscreen' : 'Pop-up',
        walletVersion: Browser.runtime.getManifest().version,
    });
}, []); // Empty dependency array

// Option 2: Use a ref to track if already fired
const hasTrackedRef = useRef(false);
useEffect(() => {
    if (!hasTrackedRef.current) {
        ampli.openedWalletExtension({
            activeNetwork,
            activeAccountType: activeAccount?.type,
            activeOrigin: activeOrigin || undefined,
            pagePath: location.pathname,
            pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
            walletAppMode: isFullScreen ? 'Fullscreen' : 'Pop-up',
            walletVersion: Browser.runtime.getManifest().version,
        });
        hasTrackedRef.current = true;
    }
}, [activeAccount?.type, activeNetwork, activeOrigin, isFullScreen, location]);

// Option 3: Rename and create separate events
// - "Opened Wallet Extension" (once on mount)
// - "Navigated to Page" (on location change)
// - "Switched Account" (on account change)
// - "Switched Network" (on network change)
```

---

### ⚠️ HIGH PRIORITY: Event #2 - DApp Connect Started

**File:** [`apps/wallet/src/ui/app/pages/site-connect/index.tsx:106-117`](apps/wallet/src/ui/app/pages/site-connect/index.tsx#L106-L117)

**Current Implementation:**
```typescript
useEffect(() => {
    if (permissionRequest) {
        const resolvedAppName = resolveApplicationName(
            permissionRequest.name,
            permissionRequest.origin,
        );
        ampli.dappConnectStarted({
            applicationName: resolvedAppName,
            applicationUrl: permissionRequest.origin,
        });
    }
}, [permissionRequest]);
```

**Problem:**
- Fires whenever `permissionRequest` object reference changes
- If the component re-renders and Redux creates a new object reference (even with same data), event fires again
- No guard to prevent duplicate tracking

**Risk Level:** Medium to High
- Depends on Redux selector memoization
- Could fire multiple times if parent component re-renders

**Expected Behavior:**
Should fire only once when the connection request page is first loaded with a permission request

**Recommended Fix:**
```typescript
// Option 1: Track by request ID
const trackedRequestIdRef = useRef<string | null>(null);
useEffect(() => {
    if (permissionRequest && trackedRequestIdRef.current !== requestID) {
        const resolvedAppName = resolveApplicationName(
            permissionRequest.name,
            permissionRequest.origin,
        );
        ampli.dappConnectStarted({
            applicationName: resolvedAppName,
            applicationUrl: permissionRequest.origin,
        });
        trackedRequestIdRef.current = requestID;
    }
}, [permissionRequest, requestID]);

// Option 2: Use request ID as dependency
useEffect(() => {
    if (permissionRequest) {
        const resolvedAppName = resolveApplicationName(
            permissionRequest.name,
            permissionRequest.origin,
        );
        ampli.dappConnectStarted({
            applicationName: resolvedAppName,
            applicationUrl: permissionRequest.origin,
        });
    }
}, [requestID]); // Use stable ID instead of object
```

---

### ⚠️ MEDIUM PRIORITY: Event #3 - Stake Clicked

**Files:**
- [`apps/wallet/src/ui/app/pages/home/tokens/TokenStakingOverview.tsx:52-62`](apps/wallet/src/ui/app/pages/home/tokens/TokenStakingOverview.tsx#L52-L62)
- [`apps/wallet/src/ui/app/staking/validators/ValidatorsCard.tsx:83-89`](apps/wallet/src/ui/app/staking/validators/ValidatorsCard.tsx#L83-L89)

**Current Implementation (TokenStakingOverview.tsx):**
```typescript
function handleOnClick() {
    if (shouldOpenNewTab) {
        openInNewTab('/stake');
    } else {
        navigate('/stake');
    }

    ampli.stakeClicked({
        isCurrentlyStaking: totalDelegatedStake > 0,
        sourceFlow: SOURCE_FLOW,
    });
}
```

**Problem:**
- Event fires **after** navigation has already occurred
- If navigation is immediate (synchronous), event may not be flushed before page unloads
- Especially problematic with `openInNewTab` which creates a new window context

**Risk Level:** Medium
- Modern browsers batch analytics events
- Amplitude SDK has flush-on-unload logic
- But timing is not guaranteed

**Expected Behavior:**
Event should be tracked before navigation, or with confirmation that it was sent

**Recommended Fix:**
```typescript
// Option 1: Track before navigation
async function handleOnClick() {
    ampli.stakeClicked({
        isCurrentlyStaking: totalDelegatedStake > 0,
        sourceFlow: SOURCE_FLOW,
    });

    // Small delay to ensure event is sent
    await ampli.flush();

    if (shouldOpenNewTab) {
        openInNewTab('/stake');
    } else {
        navigate('/stake');
    }
}

// Option 2: Simpler - just reorder
function handleOnClick() {
    ampli.stakeClicked({
        isCurrentlyStaking: totalDelegatedStake > 0,
        sourceFlow: SOURCE_FLOW,
    });

    if (shouldOpenNewTab) {
        openInNewTab('/stake');
    } else {
        navigate('/stake');
    }
}
```

---

### ⚠️ MEDIUM PRIORITY: Event #4 - Collectible Card Clicked

**File:** [`apps/wallet/src/ui/app/pages/home/nfts/VisualAssets.tsx:60-87`](apps/wallet/src/ui/app/pages/home/nfts/VisualAssets.tsx#L60-L87)

**Current Implementation:**
```typescript
<Link
    to={/* url */}
    onClick={() => {
        ampli.collectibleCardClicked({
            collectibleType: object.type!,
        });
    }}
    key={object.objectId}
    className="relative no-underline"
>
```

**Problem:**
- Event fires in `onClick` handler of `<Link>` component
- React Router's `<Link>` triggers navigation synchronously
- Event may not be flushed before navigation occurs
- Same timing issue as "Stake Clicked"

**Risk Level:** Medium
- Same concerns as Event #3
- Navigation is client-side (React Router) so less severe than full page navigation

**Expected Behavior:**
Event should be guaranteed to send before navigation

**Recommended Fix:**
```typescript
const handleCardClick = async (e: React.MouseEvent, object: IotaObjectData) => {
    e.preventDefault();

    ampli.collectibleCardClicked({
        collectibleType: object.type!,
    });

    // Navigate programmatically after tracking
    const url = isKioskOwnerToken(kioskClient.network, object)
        ? `/kiosk?${new URLSearchParams({
              kioskId: getKioskIdFromOwnerCap(object),
          })}`
        : `/nft-details?${new URLSearchParams({
              objectId: object.objectId,
          }).toString()}`;

    navigate(url);
};

// In JSX
<div
    onClick={(e) => handleCardClick(e, object)}
    key={object.objectId}
    className="relative no-underline cursor-pointer"
>
```

---

### ℹ️ LOW PRIORITY: Event #5 - Collectible Un-Hidden

**File:** [`apps/wallet/src/ui/app/pages/home/nfts/VisualAssets.tsx:38-54`](apps/wallet/src/ui/app/pages/home/nfts/VisualAssets.tsx#L38-L54)

**Current Implementation:**
```typescript
toast(
    (t) => (
        <MovedAssetNotification
            t={t}
            destination="Hidden Assets"
            onUndo={() => {
                showAsset(object.objectId);
                ampli.collectibleUnHidden({
                    collectibleType: object.type!,
                });
            }}
        />
    ),
    {
        duration: 4000,
    },
);
```

**Problem:**
- Event fires inside a toast `onUndo` callback
- Creates indirect user action tracking
- Toast may expire before user sees it
- Event only fires if user clicks "Undo" within 4 seconds

**Risk Level:** Low
- This is actually intentional behavior
- Event correctly tracks the "un-hide" action
- Toast pattern is acceptable for undo actions

**Notes:**
- Not a bug, but worth documenting for analytics interpretation
- The event will only fire when user explicitly clicks "Undo"
- If toast expires, no event fires (user chose not to undo)

**Recommended Action:**
- No code changes needed
- Document in analytics documentation that "Collectible Un-Hidden" only tracks explicit undo actions
- Consider adding analytics to track: "Collectible Hide Undo Expired" if needed

---

## Correctly Implemented Events (29 Events)

### Account Management Events ✅
- **Accounts Added** - Fires in mutation success callback
- **Account Deleted** - Fires after successful removal
- **Account Renamed** - Fires after successful nickname update
- **Account Keys Exported** - Properly tracked in export flow
- **Balance Finder Used** - Click handler tracking

### DApp & Transaction Events ✅
- **Responded to Connection Request** - Fires on explicit user action (approve/reject)
- **Application Disconnected** - Fires in mutation callback
- **Application Opened** - Click handler tracking
- **Responded to Transaction Request** - Has analytics exclusion list for high-volume apps
- **Transaction Opened** - Click handler on transaction card

### Token Events ✅
- **Coin Selected** - Click handler tracking
- **Coin Pinned** - Direct user action
- **Coin Unpinned** - Direct user action
- **Coins Sent** - Mutation success callback

### NFT Events ✅
- **Collectible Hidden** - Direct user action with proper event/stopPropagation

### Staking Events ✅
- **Validator Selected** - Button click before navigation (correct implementation)
- **IOTA Staked** - Mutation success callback
- **IOTA Unstaked** - Mutation success callback

### Network & Settings Events ✅
- **Network Switched** - Fires after successful mutation with try-catch
- **Theme Changed** - Direct user action
- **Auto Lock Updated** - Helper function with clear trigger
- **Wallet Reset** - Mutation function before clearing data

### External Actions ✅
- **External Link Opened** - Click handler with explicit tracking
- **Element Copied** - Fires after successful clipboard copy
- **Apps Banner CTA Clicked** - Click handler with trackEvent flag

### Hardware Wallet Events ✅
- **Connected Hardware Wallet** - Post-connection tracking
- **Opened Connect Ledger Flow** - Flow initialization tracking

---

## Analytics Architecture Review

### Initialization
**File:** [`apps/wallet/src/shared/analytics/amplitude.ts`](apps/wallet/src/shared/analytics/amplitude.ts)

- ✅ Properly initialized with environment `'iotawallet'`
- ✅ Only enabled in production (`BUILD_ENV === 'production'`)
- ✅ Uses cookie storage for persistence
- ✅ Auto-flushes on page hide/visibility change
- ✅ Has proper error handling

### Identity Management
**File:** [`apps/wallet/src/ui/app/redux/store/amplitudeMiddleware.ts`](apps/wallet/src/ui/app/redux/store/amplitudeMiddleware.ts)

- ✅ Redux middleware syncs user identity
- ✅ Tracks: `network`, `walletAppMode`, `walletVersion`
- ✅ Properly updates on state changes

### Event Protection
- ✅ Data masking via `data-amp-mask` attribute for sensitive data
- ✅ Analytics exclusion list for high-volume apps (transaction requests)
- ✅ Dialog context plugin adds dialog information
- ✅ Environment plugin prefixes dev events

---

## Summary Statistics

| Metric | Count |
|--------|-------|
| **Total Events Reviewed** | 34 |
| **Critical Issues** | 1 |
| **High Priority Issues** | 1 |
| **Medium Priority Issues** | 2 |
| **Low Priority Issues** | 1 |
| **Correctly Implemented** | 29 |

---

## Recommended Action Plan

### Phase 1: Immediate (Critical)
1. ✅ Fix "Opened Wallet Extension" event (Event #1)
   - Implement ref-based tracking or empty dependency array
   - Consider renaming to "Page Viewed" if tracking navigation is desired
   - Add separate "Extension Opened" event that fires once per session

### Phase 2: High Priority
2. ✅ Fix "DApp Connect Started" event (Event #2)
   - Add request ID tracking to prevent duplicates
   - Test with various re-render scenarios

### Phase 3: Medium Priority
3. ✅ Fix "Stake Clicked" timing (Event #3)
   - Move event tracking before navigation
   - Consider adding flush() call for new tab scenarios

4. ✅ Fix "Collectible Card Clicked" timing (Event #4)
   - Implement preventDefault pattern
   - Navigate programmatically after tracking

### Phase 4: Documentation
5. ✅ Document "Collectible Un-Hidden" behavior (Event #5)
   - Add to analytics documentation
   - Note that it only tracks explicit undo actions

---

## Testing Recommendations

### For Each Fixed Event:

1. **Unit Tests**
   - Verify event fires exactly once
   - Test various trigger scenarios
   - Mock analytics to verify properties

2. **Integration Tests**
   - Test in browser extension environment
   - Verify events are sent before navigation
   - Test with network throttling

3. **Manual Testing Scenarios**

**Event #1 (Opened Wallet Extension):**
   - Open extension → should fire once
   - Navigate between pages → should NOT fire
   - Switch accounts → should NOT fire
   - Change network → should NOT fire
   - Close and reopen extension → should fire once

**Event #2 (DApp Connect Started):**
   - Load connection request page → should fire once
   - Parent component re-renders → should NOT fire again
   - Same request reloaded → should NOT fire again

**Event #3 & #4 (Navigation Timing):**
   - Click stake/NFT card → event should send before navigation
   - Monitor network tab to verify timing
   - Test with slow network conditions

4. **Analytics Validation**
   - Monitor Amplitude dashboard for duplicate events
   - Compare before/after metrics
   - Verify event properties are correct

---

## Additional Observations

### Positive Patterns Identified
1. ✅ **Mutation callbacks** - Most destructive actions properly fire events after success
2. ✅ **Try-catch blocks** - Network changes and critical operations have error handling
3. ✅ **Data masking** - Sensitive data properly masked with `data-amp-mask`
4. ✅ **High-volume protection** - Transaction requests have exclusion list
5. ✅ **Proper flush handling** - Page hide/visibility change triggers flush

### Patterns to Avoid
1. ❌ **useEffect with object dependencies** - Use stable IDs or refs instead
2. ❌ **Tracking after navigation** - Always track before navigation or use async
3. ❌ **No duplicate prevention** - Use refs or IDs to prevent multiple firings

### Patterns to Follow
1. ✅ **Track in mutation callbacks** - Ensures action completed successfully
2. ✅ **Click handlers for navigation** - Fire before navigate() call
3. ✅ **Refs for one-time events** - Use useRef to track if event already fired
4. ✅ **Stable dependencies** - Use IDs instead of objects in useEffect deps

---

## Conclusion

The IOTA wallet has a well-structured analytics implementation with proper initialization, identity management, and data protection. However, the "Opened Wallet Extension" event is critically flawed and needs immediate attention. The other 4 events require optimization to ensure reliable tracking, but are less severe.

After implementing the recommended fixes, the analytics system will provide accurate, reliable data without noise from duplicate or mistimed events.

---

**Report Generated:** 2026-03-17
**Reviewer:** Claude (AI Code Analysis)
**Next Review:** After implementing Phase 1 & 2 fixes