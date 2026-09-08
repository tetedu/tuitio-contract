# How it works

## The two contracts

**Institution registry** (`CD2INHSYNIQTVWM222CNSS7MN4MJSOBZOXVEWY3SOAK6MHICCZIEVQWV` on testnet)
answers one question: is this address a legitimate tuition payee? Anyone can
register; records start `Pending`. The registry admin — a verification role,
deliberately separate from the escrow admin — promotes institutions to
`Verified`. Only `Verified` institutions can be named in grants. Suspension
blocks new grants and makes the payout lookup trap, so a suspended school
cannot be paid even for terms already attested.

**Tuition escrow** (`CDV7FA3QJPBR7LORHCZCZG6UCRL7DMXVMZYRDLMW7EXUNBMN7SEXINXW` on testnet)
holds the funds and settles terms.

## Grant lifecycle

Creating a grant transfers the **entire commitment** —
`term_amount × terms_total` — into the contract. A grant is fully
collateralised from the moment it exists; the sponsor cannot promise money
they do not have, and later terms cannot fail for lack of funds.

Each term then walks this state machine:

```
Pending ──attest──▶ Attested ──(dispute window closes)──▶ Released
                        │
                     dispute
                        ▼
                     Disputed ──admin resolve──▶ Released or Refunded
```

1. **Attest.** The institution calls `attest_term` for the *next* term in
   order — it cannot claim term 5 while term 2 is unresolved. Attesting moves
   no money. It starts a dispute window (7 days on testnet).
2. **Dispute window.** The sponsor can call `dispute_term` to freeze the term
   before the window closes. After the window closes, no dispute is possible.
3. **Release.** Once the window has elapsed without dispute, *anyone* can
   call `release_term` — permissionless on purpose, so no party can stall an
   owed payment by withholding a signature. The contract looks up the
   institution's registered payout address and pays it.
4. **Dispute resolution.** A disputed term is settled by the escrow admin:
   pay the institution or refund the sponsor. Either way the term is final
   and the grant advances.
5. **Cancellation.** The sponsor can cancel a grant and receive every
   unsettled term back in one transfer. Cancellation is refused while any
   term is attested or disputed — the institution has done work it is claiming
   for, and the sponsor cannot pull funds out from under an open claim.

When the last term settles, the grant completes.

## The conflict of interest, stated plainly

The institution both attests that a term happened *and* is the party getting
paid. That is a real conflict, and Tuitio does not pretend an oracle removes
it. What constrains it:

- Attesting moves no money — it starts a clock.
- The sponsor can freeze the term during the window.
- Registry suspension freezes payment even after attestation.
- Terms settle strictly in order, so bogus claims cannot be buried.

A decentralized "did this student attend" oracle does not exist on Stellar,
and faking one would be worse than the honest design.

## A worked example (the live testnet demo)

- Sponsor funds a 3-term grant at 50 XLM per term: **150 XLM enters escrow**
  in the creation transaction.
- The institution attests term 0. The dashboard shows "Attested — dispute
  window open" for 7 days.
- A second 2-term grant shows the full dispute path: attested → disputed by
  the sponsor → resolved by the admin in the institution's favour → **50 XLM
  paid out**, term 1 awaiting its turn, 50 XLM still locked.
