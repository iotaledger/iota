# Interface: PasskeyAuthenticator

An passkey authenticator with parsed fields. See field definition below. Can be initialized from
[struct RawPasskeyAuthenticator].

## Properties

### authenticator\_data

> **authenticator\_data**: `number`[]

`authenticatorData` is a bytearray that encodes
[Authenticator Data](https://www.w3.org/TR/webauthn-2/#sctn-authenticator-data) structure returned
by the authenticator attestation response as is.

---

### client\_data\_json

> **client\_data\_json**: `string`

`clientDataJSON` contains a JSON-compatible UTF-8 encoded string of the client data which is passed
to the authenticator by the client during the authentication request (see
[CollectedClientData](https://www.w3.org/TR/webauthn-2/#dictdef-collectedclientdata))
