# 0003. JSON, canonicalized by RFC 8785, named by SHA-256

**Context.** Changes and blobs need names that anyone can verify, and records must be readable by
every language and every AI model without special libraries.

**Decision.** Records and changes are JSON. Content IDs are `sha256:<hex>` of the bytes; for JSON,
of its RFC 8785 canonical form (JCS). Signatures are Ed25519 over the content ID.

**Consequences.** Every language has JSON and SHA-256; `sha256sum` checks a blob. JCS needs a
small library in some languages and care with numbers, which is one more reason exact amounts are
strings (spec §1.2). JSON is bigger than binary formats; stores may compress.

**Alternatives.** CBOR / DAG-CBOR (as in IPFS and AT Protocol): compact and canonical, but opaque
to people and to models reading raw data. Protocol Buffers: needs generated code. Multihash
prefixes for future hash algorithms: deferred; `sha256:` is already a prefix, so a new algorithm
is a new prefix.
