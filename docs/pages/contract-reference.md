# Smart contract reference

Both contracts are deployed on Stellar testnet. Errors are `contracterror`
enums; values below are the on-chain codes.

## institution-registry

Errors: `AlreadyInitialized=1, NotInitialized=2, NotAdmin=3,
InstitutionNotFound=4, InstitutionExists=5, InvalidStatus=6, InvalidMetadata=7`

### `initialize(admin: Address) -> Result<(), Error>`
Sets the registry admin. Callable once. **Auth:** none (deployment-time).

### `register(caller: Address, payout: Address, name: String, country: String) -> Result<(), Error>`
Self-register as an institution, starting `Pending`. `name` is 1–128 bytes;
`country` is exactly 2 bytes (ISO alpha-2). **Auth:** `caller`. Emits
`institution_registered`.

### `verify(institution: Address) -> Result<(), Error>`
Promote a `Pending` or `Suspended` institution to `Verified`. **Auth:** registry
admin. Emits `institution_verified`.

### `suspend(institution: Address) -> Result<(), Error>`
Move a `Verified` institution to `Suspended`. Existing grants are untouched;
new grants naming the institution are rejected. **Auth:** registry admin.
Emits `institution_suspended`.

### `update_payout(institution: Address, new_payout: Address) -> Result<(), Error>`
Rotate the address that receives tuition. **Auth:** the institution's own
admin address. Emits `payout_updated`.

### `set_admin(new_admin: Address) -> Result<(), Error>`
Transfer registry administration. **Auth:** current admin. Emits
`admin_changed`.

### `get_institution(institution: Address) -> Result<Institution, Error>`
Read. Returns `{admin, payout, name, country, status, registered_at}`.

### `payout_of(institution: Address) -> Result<Address, Error>`
Read, but only for `Verified` institutions — traps with `InvalidStatus`
otherwise. This is what the escrow calls before paying.

### `is_verified(institution: Address) -> bool`
Read. `false` for unknown or non-verified addresses, never an error.

### `list(start: u32, limit: u32) -> Vec<Address>`
Read. Institution addresses in registration order. `start` is a registration
position, not an id, and positions are append-only, so a page stays stable as
the registry grows. `limit` is clamped to 100; a `start` past the end returns
an empty vector rather than an error, so a caller can page until the result is
short. Every record is indexed regardless of status — filtering is the
caller's decision.

### `count() -> u32` · `admin() -> Result<Address, Error>`
Reads.

## tuition-escrow

Errors: `AlreadyInitialized=1, NotInitialized=2, GrantNotFound=3,
TermNotFound=4, InstitutionNotVerified=5, InvalidAmount=6, InvalidTermCount=7,
GrantNotActive=8, TermNotPending=9, TermNotAttested=10, TermNotDisputed=11,
OutOfOrderTerm=12, DisputeWindowOpen=13, DisputeWindowClosed=14, Overflow=15,
SettlementInFlight=16, InvalidDisputeWindow=17`

### `initialize(admin: Address, registry: Address, dispute_window: u64) -> Result<(), Error>`
Configure the escrow. `admin` resolves disputes (separate role from the
registry admin by design). `dispute_window` is in seconds and must be
non-zero. **Auth:** none (deployment-time).

### `create_grant(sponsor, beneficiary, institution, token_id, term_amount: i128, terms_total: u32) -> Result<u64, Error>`
Creates a grant and pulls `term_amount × terms_total` from the sponsor into
the contract. Rejects unverified institutions, non-positive amounts, zero
terms, and overflow. **Auth:** `sponsor`. Emits `grant_created`. Returns the
grant id.

### `attest_term(grant_id: u64, term_index: u32) -> Result<(), Error>`
The institution attests a completed term. Only the grant's *next* term is
accepted. Moves no funds; starts the dispute window. **Auth:** the grant's
institution. Emits `term_attested`.

### `release_term(grant_id: u64, term_index: u32) -> Result<(), Error>`
Pays an attested term to the institution's registered payout address, once
the dispute window has elapsed. Traps if the institution has been suspended
since attestation. **Auth:** none — permissionless. Emits `term_released`.

### `sweep(grant_ids: Vec<u64>) -> Result<u32, Error>`
Settles every grant in the batch whose current term is attested and past its
dispute window, returning how many were released. Permissionless, like
`release_term`. Entries that are not due are skipped rather than failing the
batch: cancelled grants, unattested or still-disputable terms, unknown ids,
and institutions suspended after attesting — that last case would otherwise
trap and revert every other settlement in the same transaction. Rejects a
batch larger than 20 with `BatchTooLarge`.

### `dispute_term(grant_id: u64, term_index: u32) -> Result<(), Error>`
Freezes an attested term before its window closes. **Auth:** the grant's
sponsor. Emits `term_disputed`.

### `resolve_dispute(grant_id: u64, term_index: u32, release: bool) -> Result<(), Error>`
Admin settles a disputed term: `release=true` pays the institution,
`release=false` refunds the sponsor. The term becomes final and the grant
advances either way. **Auth:** escrow admin. Emits `term_refunded` (refund
path) and `dispute_resolved`.

### `cancel_grant(grant_id: u64) -> Result<(), Error>`
Refunds every unsettled term (`term_amount × (terms_total − next_term)`) and
closes the grant. Refused while a term is attested or disputed. **Auth:** the
grant's sponsor. Emits `grant_cancelled`.

### Reads
- `get_grant(grant_id) -> Result<Grant, Error>`
- `get_term(grant_id, term_index) -> Result<Term, Error>` — unattested terms
  read as `Pending`
- `locked_amount(grant_id) -> Result<i128, Error>` — amount still in escrow
- `get_config() -> Result<Config, Error>` · `next_grant_id() -> u64`

## Events

Every state change emits a typed event (`registry_initialized`, `escrow_initialized`, `institution_registered`,
`grant_created`, `term_attested`, `term_released`, `term_disputed`,
`dispute_resolved`, `term_refunded`, `grant_cancelled`, `grant_completed`,
`institution_verified`, `institution_suspended`, `payout_updated`,
`admin_changed`). The indexer decodes these to build the read model; field
names in events are a public interface for downstream consumers.
