# Sponsor guide

You are paying for a student's education and want the money to reach the
school, not become general household cash.

## What you need

- A Stellar wallet ([Freighter](https://freighter.app) works with the Tuitio
  web app) funded with the tuition token.
- A verified institution to name in the grant — see the Institutions page.

## Funding a grant

1. Open **Fund a grant** in the web app and connect your wallet.
2. Pick the institution, enter the student's address, the amount per term and
   the number of terms.
3. Confirm in your wallet. The **entire commitment** — per-term amount ×
   terms — leaves your wallet immediately and enters the escrow contract.
   You will not be asked for money again for later terms.

You can watch the grant on the dashboard: total committed, what has settled,
and what is still in escrow.

## What happens each term

The institution attests that the term was completed. Nothing is paid yet — a
dispute window opens (7 days on testnet). During the window:

- **If everything is fine**, do nothing. Once the window closes the term
  releases to the school automatically — anyone can trigger the transfer, so
  it cannot be stalled.
- **If the student dropped out or something is wrong**, open the grant page
  and press **Dispute this term**. The term freezes and the protocol admin
  resolves it: either the term is paid anyway (it really happened) or your
  money for that term comes back to you.

## Getting out

You can cancel a grant at any time as long as no term is currently attested
or disputed. Every unsettled term refunds to your wallet in one transfer.
Terms already released stay released — they paid for terms that happened.

## What you cannot do

- You cannot redirect escrowed funds to anyone but the institution.
- You cannot cancel while a term you have been notified about (attested) is
  unresolved — the school may already have delivered that term.
