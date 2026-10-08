# SmartAccount Allowed Authenticators Move Example

An extension for `0x2::smart_account::SmartAccount` that limits the authenticators an account can rotate to. It uses the framework's rotation rules (`0x2::smart_account_rotation_rules`): the `allowed_authenticators` module attaches the rule `AllowedAuthenticatorsRule`, whose config is the list of allowed authenticators, and approves a rotation only to an authenticator in that list.

The list can be attached:

- while building an account, with `allowed_authenticators::with_allowed_authenticators`, e.g. on top of `0x2::smart_account_builtin_auth::builder_v1`;
- to an account that already exists, with `allowed_authenticators::attach_allowed_authenticators`, in a transaction sent by the account.

The list can only shrink (`disallow_authenticator`) and the rule can't be removed, so whoever controls the account's current authenticator can't widen it. Include the built-in authenticator in the list for the account to be able to rotate back to it.

The package also holds two authenticators for `SmartAccount` to fill the list, adapted from the other examples in this directory:

- `ed25519_authenticator`, from `public_key_authentication`: checks an Ed25519 signature of the transaction digest against the public key attached with `0x2::smart_account_public_key`, the same key the built-in authenticator reads;
- `time_locked_authenticator`, from `time_locked`: the same check, once the epoch timestamp reaches an unlock time stored on the account.

Those examples' helpers take the account's `UID`, which only the framework can reach for a `SmartAccount`, so these two read the account through the `SmartAccount` functions instead.

## Rotating the authenticator

With the list attached, `0x2::smart_account::rotate_auth_function_ref_v1` aborts and a rotation needs a receipt from every rule attached to the account. For an account built with `smart_account_builtin_auth`, that is `BuiltinAuthRule` and `AllowedAuthenticatorsRule`:

```move
let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
    &account,
    new_authenticator,
    ctx,
);
smart_account_builtin_auth::approve_auth_rotation(&account, &mut request);
allowed_authenticators::approve_auth_rotation(&account, &mut request);
smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(&mut account, request, ctx);
```

`allowed_authenticators::rotate_auth_function_ref_v1` does this in one call when the only other rule, if any, is `BuiltinAuthRule`.

## How to run the tests

```bash
iota move test --path examples/move/abstract_iota_accounts/smart_account_allowed_authenticators
```
