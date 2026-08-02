# Telegram login-code delivery in third-party clients

Research snapshot: 2026-08-01

## Conclusion

An `auth.sentCode` response means Telegram accepted the authorization request and selected a delivery flow; it does not mean an SMS was sent. Telegram chooses the method and reports it in `auth.sentCode.type`. For `auth.sentCodeTypeApp`, the code is sent as a Telegram service notification to every *other* logged-in session. Telegram's FAQ tells users to find it in the verified **Telegram** service chat, not in an SMS inbox. ([Telegram authorization documentation](https://core.telegram.org/api/auth#code-types), [Telegram FAQ](https://telegram.org/faq#login-and-sms))

The most consequential policy restriction is that Firebase-backed SMS authentication is available only to official mobile clients. Telegram explicitly says that third-party apps may be unable to receive a login or signup code by SMS/call under conditions where Firebase SMS is the only applicable method. Third-party clients may instead use Telegram in-app codes, Fragment, email, future authorization tokens, or QR login. Developers who require SMS can ask Telegram to enable it for their API application by emailing `sms@telegram.org` with `#enableSMS` in the subject. ([Telegram authorization documentation](https://core.telegram.org/api/auth#code-types))

Therefore, if Popgram reports **Telegram app delivery** and the account has no reachable logged-in session, repeatedly calling `auth.sendCode` cannot force an SMS and is unlikely to help. The practical recovery path is to establish or recover an official mobile session, then either read the service-notification code or authorize Popgram with QR login. Telegram's QR flow requires an already logged-in app to scan and accept the token. ([Telegram QR-login flow](https://core.telegram.org/api/qr-login), [Telegram FAQ](https://telegram.org/faq#login-and-sms))

## What the server response means

`auth.sentCode` includes four important values: the current delivery `type`, a private `phone_code_hash` needed for later calls, an optional `next_type`, and an optional `timeout`. Telegram says the system automatically chooses the delivery method. The client should display the current method and retain the other fields; it must not imply that an SMS was sent merely because the RPC succeeded. ([Telegram authorization documentation](https://core.telegram.org/api/auth#code-types), [`auth.SentCodeType`](https://core.telegram.org/type/auth.SentCodeType))

When `timeout` has elapsed, `auth.resendCode` may request the server-advertised `next_type`. It does not let a client choose an arbitrary medium or force SMS. Repeated resends can end with `SEND_CODE_UNAVAILABLE` after the server's available delivery options are exhausted. ([Telegram authorization documentation](https://core.telegram.org/api/auth#code-types), [`auth.resendCode`](https://core.telegram.org/method/auth.resendCode))

The current delivery types include Telegram app, SMS, call, flash call, missed call, configured email, email setup, Fragment, Firebase SMS, SMS word, and SMS phrase. In particular, `auth.sentCodeTypeApp` is defined as delivery through the Telegram app, and Telegram's authorization guide specifies that the service notification goes to all other logged-in sessions. ([`auth.SentCodeType`](https://core.telegram.org/type/auth.SentCodeType), [Telegram authorization documentation](https://core.telegram.org/api/auth#code-types))

Telegram documents a per-phone-number daily login-attempt limit (its example is five, but it explicitly says the value can change). Production retries should therefore stop once the current request is accepted, wait for the advertised timeout before resend, and avoid repeated clean-session experiments. Telegram recommends validating authorization logic against its reserved test accounts on test data centers before production testing. ([Telegram test-account documentation](https://core.telegram.org/api/auth#test-accounts))

## Comparison with Grammers 0.10

The old Grammers high-level request does not contain a hidden SMS-enabling setting. Its `request_login_code` builds `auth.sendCode` with `allow_flashcall`, `current_number`, `allow_app_hash`, `allow_missed_call`, `allow_firebase`, and `unknown_number` all `false`; `logout_tokens`, `token`, and `app_sandbox` are absent. It handles an RPC 303 migration by changing the home DC and retrying the same request. ([Grammers 0.10 `request_login_code` source](https://docs.rs/crate/grammers-client/0.10.0/source/src/client/auth.rs#242-292))

Popgram currently sends the same `CodeSettings` values and performs the same `PHONE_MIGRATE` retry at the application boundary. Consequently, there is no request-field difference that explains Grammers forcing a code while Popgram does not. The current Popgram flow is actually richer at this layer: it preserves the returned delivery type, fallback type, and timeout, and it exposes `auth.resendCode`; Grammers 0.10's high-level method keeps only the phone number and `phone_code_hash`. ([Popgram request implementation](../../crates/popgram-telegram/src/lib.rs), [Grammers 0.10 source](https://docs.rs/crate/grammers-client/0.10.0/source/src/client/auth.rs#242-295))

Grammers and Popgram both use `invokeWithLayer(initConnection(...))`, but their client metadata and transport implementations differ. There is no official source saying those differences change code delivery after Telegram has returned `auth.sentCode`, so treating them as the cause would be speculation. Telegram documents that it, not the client, automatically selects the delivery method. ([Telegram authorization documentation](https://core.telegram.org/api/auth#code-types), [`initConnection`](https://core.telegram.org/method/initConnection))

Likewise, deferring the MTProto acknowledgment until the next RPC is normal protocol behavior, not evidence that Telegram rolled back code delivery. Telegram explicitly says a client normally adds the acknowledgment for an RPC response to its next query, unless the delay or pending count warrants a standalone acknowledgment. ([MTProto acknowledgment rules](https://core.telegram.org/mtproto/service_messages_about_messages#acknowledgment-of-receipt))

## How to distinguish policy from an implementation bug

For one request only, record the following non-secret diagnostics from both a genuinely fresh Grammers session and a genuinely fresh Popgram session, using the same phone number, API credentials, network, and approximate time:

- the returned `SentCodeType` constructor;
- `next_type` and `timeout`;
- the destination DC and any migration RPC error;
- any subsequent RPC error from `auth.resendCode` after the timeout.

Do not log the phone number, API hash, `phone_code_hash`, login code, authorization key, or message contents.

The comparison must use fresh unauthenticated Grammers state. Grammers' normal example calls `is_authorized` before requesting a code; a persisted authorized Grammers session proves only that its previous authorization still works, not that a new code was delivered. ([Grammers `Client` authentication documentation](https://docs.rs/grammers-client/0.10.0/grammers_client/client/struct.Client.html#method.is_authorized))

Interpret the result as follows:

- `App`: check the verified **Telegram** service chat on every other active session. If none is reachable, prioritize QR login support and recovery through an official mobile app.
- `Email`, `Fragment`, `SmsWord`, or `SmsPhrase`: follow the returned method exactly; an SMS-number prompt is incorrect for some of these constructors.
- `FirebaseSms`: Popgram cannot complete the official-only attestation flow as a third-party client.
- A populated `next_type`: enable resend only after `timeout`; do not label it SMS unless `next_type` says SMS.
- Identical metadata in fresh Grammers and Popgram runs: the evidence points to Telegram delivery/account policy rather than Popgram's `auth.sendCode` encoding.
- Different metadata under otherwise controlled conditions: capture the non-secret `initConnection` identity fields and DC selection next, but keep the result classified as an observation rather than a proven server-selection rule.

## Product implications

Popgram should make QR login a first-class path for existing accounts. It should render the exact server-selected delivery method, destination hint, code length, fallback method, and countdown; enable resend only when Telegram permits it; and explain that third-party clients cannot generally force SMS. QR login is useful only when another Telegram session is already authorized, because that session must scan and accept the token. ([Telegram QR-login flow](https://core.telegram.org/api/qr-login))

Popgram should not implement `auth.reportMissingCode` as a remedy: Telegram marks that RPC as official-app-only. ([`auth.reportMissingCode`](https://core.telegram.org/method/auth.reportMissingCode))
