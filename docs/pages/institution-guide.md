# Institution guide

You are a school or training provider that wants reliable, term-by-term
tuition payment from sponsors abroad.

## Getting registered

Registration is permissionless: any wallet can submit your institution's
details, starting as `Pending`. To appear in the web app's grant flow you
must be **`Verified`** — the registry admin reviews pending registrations
(name, country, payout address) and promotes them. Contact the maintainer
(adelekevat@gmail.com) for testnet verification.

Your registration carries two addresses:

- **Admin address** — registered the institution, controls payout rotation.
- **Payout address** — the only address tuition can ever be sent to. It can
  be rotated with `update_payout`, signed by your admin address.

## Attesting terms

When a sponsor funds a grant naming your institution, the full amount sits
in escrow. After each academic term:

1. Open the grant (or use your institution dashboard) with the wallet that is
   your institution address.
2. Press **Attest term N**. This states the student completed the term.
3. A dispute window runs (7 days on testnet). If the sponsor does not object,
   the term's funds release to your payout address — automatically, without
   you chasing anyone.

You must attest terms **in order**. You cannot claim term 3 while term 2 is
unresolved.

## Disputes

A sponsor can dispute an attested term during the window. The term freezes
and the protocol admin decides: the term pays out, or it refunds to the
sponsor. Either way the grant moves on to the next term.

## Suspension

If the registry admin suspends your institution, new grants naming you are
rejected, and attested-but-unreleased terms cannot pay out until the dispute
is resolved. Suspension is reversible.
