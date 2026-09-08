# Introduction

## What Tuitio is

Tuitio is milestone-gated tuition escrow on Stellar. A sponsor — a parent
abroad, a scholarship fund, a church congregation — commits money for a
student's academic terms into a Soroban smart contract. The money is bound to
the institution: the student is recorded as the beneficiary but can never
receive the funds, and the only address the contract will ever pay is the
school's registered payout address. Funds release one term at a time, after
the institution attests the term happened and a dispute window closes.

## The problem, with numbers

Cross-border education funding is large and leaky. UNESCO estimates
international students' families spend well over \$100 billion a year on
education abroad, and remittances — a common channel for school fees —
reached about \$685 billion in 2024, with sub-Saharan Africa among the
fastest-growing corridors. But money sent as general remittance arrives as
general cash: when a household faces a medical bill or a food shortfall the
same week school fees are due, spending the school money is the rational
choice. Studies of education-focused conditional transfers consistently find
earmarking improves school spending versus unrestricted cash. The sponsor has
no way to verify the money reached the school, and chasing receipts across
borders is manual and slow.

Tuitio makes the earmark enforceable: the money physically cannot become
household cash, because it never leaves escrow except to the institution.

## Why Stellar specifically

- **Stablecoin rails.** USDC on Stellar and the anchor network make the
  corridor work: value crosses borders as a stablecoin and settles out to
  local currency, which is the actual shape of school-fee payments.
- **Cost.** At Stellar fee levels (fractions of a cent), escrowing and
  releasing a single \$40-per-term fee is economically sensible. On chains
  where a transaction costs dollars, the design would be absurd.
- **Soroban.** The escrow needs real contract logic — authorization, dispute
  windows, ordered settlement — not a token swap.

## Where things stand

Everything in these docs is live on Stellar **testnet**. The contracts are
unaudited; the protocol is a working MVP, not a production system.
