# Core-shaped types

**Abstract:**
*Ethos* expresses Bitcoin Core’s JSON-RPC surface as a structured description. By aligning observed RPC behavior of the running system (*B*) with a machine-readable model [(*Δ*)](../resources/ir/bitcoin.ir.json), we make that surface self-describing and enduring as Core-shaped types.

## 1. Introduction

The Bitcoin system is governed not only by its consensus rules, but also by the interfaces and procedures that define how participants interact with those rules. Bitcoin’s consensus rules are defined by software and secured by cryptography. The interfaces that expose those rules, however, are still described informally in code and documentation.

**Core-shaped types** come from reducing the gap between *Bitcoin Core RPC as it exists* and *Bitcoin Core RPC as it is described.*
We represent this as the convergence of two entities:

- **B** — Bitcoin Core’s observed JSON-RPC behavior for a pinned release.
- **Δ** — the *structured schema*, an explicit, machine-readable representation of that RPC surface.



## 2. Behavioral vs. Structured Description

Bitcoin Core defines RPC behavior through implementation; its behavior *is* the reference for clients. The task of convergence is to express that surface as a structured schema that can be shared, versioned, and used to generate type-safe clients.

The structured schema Δ, by contrast, is explicit.
It defines the same RPC shapes in a declarative form that can be checked for consistency across releases.

Convergence is the continual alignment of Δ with B, capturing in schema what is already true on the wire.

## 3. Measuring Convergence

Convergence can be observed wherever the structured schema and observed RPC behavior disagree. This difference is measurable across several fronts:

- **Behavioral divergence** — when the inputs or outputs of an RPC call differ from those predicted by Δ.
- **Semantic drift** — when new software versions change behaviors without explicit schema updates.

Each instance of disagreement provides information. By recording and analyzing these discrepancies, Ethos refines Δ, reducing the behavioral distance to B.

## 4. Correctness and Verification

If a generated client conforms to Δ, and Δ accurately represents B,
then the client matches Bitcoin Core’s JSON-RPC wire for that tip.

Verification shifts from individual programs to the shared schema. Each refinement step strengthens the bridge between behavior and description.

## 5. Bounded ε

Through empirical observation and verification, the difference between B and Δ approaches a small, measurable constant:

```math
|B - Δ| \to ε
```

where ε represents the narrow band of behaviors not yet captured.

## 6. Summary

Bitcoin Core’s RPC began as code and help text. Core-shaped types are that implicit surface made explicit as schema and generated Rust.

By continually reducing the difference between observed RPC behavior (B) and its *structured representation* (Δ), Ethos seeks to make that surface self-describing and enduring for type-safe clients.

---

