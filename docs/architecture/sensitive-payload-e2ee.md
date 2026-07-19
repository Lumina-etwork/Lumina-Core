# Sensitive Payload Field E2EE Architecture

Sensitive payload fields are encrypted at the producing endpoint before a
payload crosses a service boundary. Services may keep non-sensitive routing
metadata in clear text, but values classified as `Sensitive` must be replaced
with an authenticated `EncryptedField` envelope.

## Flow

1. Classify payload fields as `Public` or `Sensitive`.
2. Derive a field key from endpoint-only shared secret material and the
   authenticated field context: service, payload type, field path, and recipient.
3. Encrypt only the sensitive field value and attach the key id, nonce,
   ciphertext, and authentication tag.
4. Downstream services route on public metadata and never log plaintext.
5. The recipient endpoint authenticates the envelope and decrypts the field.

## Security properties

- Field context is authenticated to prevent ciphertext replay into a different
  field, payload type, service, or recipient.
- Tampering is rejected before plaintext is returned.
- Audit trails and metrics must record only key ids, field paths, payload types,
  and failure counters; plaintext values and raw shared secrets are forbidden.

## Performance and availability

The implementation is allocation-bounded to the encrypted field length and is
intended for sub-100ms P99 critical paths. Deployments should use blue-green
rollout with canary analysis over encryption latency, authentication failures,
and payload rejection rate before promoting all traffic.
