# spawn-at — Pre-Spawn Group Coordination & Unified Priority-FIFO System

**Master Architectural Blueprint · Final Synthesis Revision**
Target platform: GNOME Shell / Mutter on Wayland · CLI: Rust · IPC: Unix Domain Socket + D-Bus
Supersedes: `pre_spawn_group_coordination_system_architecture (1)` and `(2)`, and the intermediate "unified master" draft.

> **Reading guide.** Sections 3–8 are the core protocol. Section 9 specifies the compositor-side (GNOME Shell extension) half. Section 10 covers failure domains. Section 11 is the complete edge-case register (the canonical 10 plus hardening additions). Section 13 is the full `config.toml`. Where this revision *changes or clarifies* an earlier draft, the change is recorded in the table in §0.3 and referenced inline as **[R#]**.

---

## Table of Contents

0. [Conventions, Terminology & Resolved Ambiguities](#0-conventions-terminology--resolved-ambiguities)
1. [Problem Statement, Design Goals & System Invariants](#1-problem-statement-design-goals--system-invariants)
2. [Architecture Overview](#2-architecture-overview)
3. [The Pre-Spawn Assembly Gate & Universal FIFO Pipeline](#3-the-pre-spawn-assembly-gate--universal-fifo-pipeline)
4. [Execution Modes & Shell Concurrency Semantics](#4-execution-modes--shell-concurrency-semantics)
5. [Inverted Priority Preemption Queue with Integer Compaction](#5-inverted-priority-preemption-queue-with-integer-compaction)
6. [Out-of-Band Execution (`--bypass-fifo`) & Wayland Focus Arbitration](#6-out-of-band-execution---bypass-fifo--wayland-focus-arbitration)
7. [CLI Flag Taxonomy & Authority Hierarchy](#7-cli-flag-taxonomy--authority-hierarchy)
8. [Concurrent Leader Arbitration & Conflict Policies](#8-concurrent-leader-arbitration--conflict-policies)
9. [Compositor-Side Specification (GNOME Shell Extension)](#9-compositor-side-specification-gnome-shell-extension)
10. [Failure Domains & Recovery](#10-failure-domains--recovery)
11. [Comprehensive Edge-Case Register](#11-comprehensive-edge-case-register)
12. [Observability & Verification](#12-observability--verification)
13. [Configuration Reference (`config.toml`)](#13-configuration-reference-configtoml)

---

## 0. Conventions, Terminology & Resolved Ambiguities

### 0.1 Normative Language

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** carry their RFC 2119 meaning. All timing uses `CLOCK_MONOTONIC` (never wall-clock). All "ordering" claims are defined with respect to the coordinator's **linearization point** (§8.1): the instant a complete handshake frame is decoded by the single coordinator thread.

### 0.2 Terminology

| Term | Definition |
| :--- | :--- |
| **Transaction** | The unit of scheduling. Exactly one of: *Solitary* (size 1), *Auto-Group* (merged `&` burst), or *Explicit Group* (`--group`). |
| **Member** | One `spawn-at` CLI invocation inside a transaction. |
| **Leader / Worker** | The member that carries `--group-size` (first to do so wins) / any other member. Only leaders hold contract authority. |
| **Contract** | The immutable terms set by the leader: target size $N$, traffic policy (`--sideline`/`--hold`), `--group-timeout`, `--hold-timeout`. |
| **Lane** | `Priority` (explicit `--priority`) or `Baseline` (default). The Priority lane always dispatches before the Baseline lane. |
| **Ready** | A transaction whose contract is satisfied (or sealed) and which is eligible for dispatch. |
| **Registering / Sealed-Waiting** | Sub-states of Phase 1. *Registering*: a bounded debounce window is open. *Sealed-Waiting*: the contract is locked and the transaction awaits remaining members. |
| **Batch** | The instruction set for one Ready transaction, sent to the extension in a single D-Bus call. |
| **Claim** | The extension-side record that says "the next window matching *this identity* belongs to *this batch entry*." |
| **Cloak / Uncloak** | Holding a mapped window at `opacity = 0` (input-inert) / releasing it via a $0 \to 255$ ramp. |
| **Settle** | The compositor-side critical section: configure-ack, geometry clamp, anchoring, uncloak, focus handoff. |
| **Lease** | A time-bounded hold on `focus_execution_lock`. |

### 0.3 Resolved Ambiguities & Design Corrections

Earlier drafts contained statements that were individually reasonable but mutually inconsistent, or that did not survive contact with real shell/Mutter behavior. Each is resolved once, here, and the resolution is applied throughout.

| ID | Earlier statement | Problem | Resolution in this revision |
| :--- | :--- | :--- | :--- |
| **R1** | Auto-groups are detected via "shared parent PGID". | In **interactive** shells with job control, every `&` job is placed in its **own** process group. `cmd1 & cmd2 &` therefore yields two different PGIDs; a PGID-equality gate would never merge them. Non-interactive scripts *do* share a PGID. | Burst key is $(\text{uid}, \text{SID}, \text{PPID})$; PGID is recorded as a diagnostic only. PPID must not be the compositor or session manager (§4.2). |
| **R2** | "Every command passes through one FIFO — nothing ever jumps the queue" **and** "`--sideline` lets unrelated spawns bypass a waiting group." | Contradictory as written. | **Ready-Set Dispatch**: FIFO order governs *Ready* transactions. A Sealed-Waiting (sidelined) group is not Ready and does not occupy the head. A *Registering* debounce window, being short and bounded, **fences** later baseline arrivals; a `--hold` group fences them until Ready or hold expiry (§3.5). |
| **R3** | Phase 2 (arm + spawn + map + clamp + uncloak) is wholly non-preemptible "~15–30 ms". | The 15–30 ms figure is compositor *settle* time. Process start-to-first-map (e.g., Electron/`code`) can take seconds. Holding the lock across it would re-create the very queue-freeze this system exists to prevent. | **Lock Split**: the lock covers *Arm+Spawn* (2A–2B) and *Settle* (2D). *Map-Await* (2C) is outside the lock (§3.3). |
| **R4** | `--hold-timeout` expiry "aborts" the group (v1) vs. "breaks the hold" (chat). | Ambiguous: does expiry kill the transaction or only release the global freeze? | Default `hold_expiry_policy = "release_and_degrade"`: the freeze is lifted, the group continues as sidelined until `group_timeout`. `"abort_group"` reproduces v1 (§7.4). |
| **R5** | $P_{\text{assigned}} = P_{\max} + K$ "prevents starvation"; $K$ expresses priority. | Because $K \ge 0$ and every new task is placed at or beyond $P_{\max}$, **no priority-lane task can ever overtake an earlier one**. $K$ therefore does not reorder the lane; it spaces entries. | Documented as the **Order-Preservation Property** (§5.3). The *effect* of `--priority` is lane membership + ordering by arrival within the lane; a baseline-starvation guard is added (§5.6). |
| **R6** | "Layer Shell / Input Grab tools bypass window management" (`grimblast`, `slurp`). | **Mutter does not implement `wlr-layer-shell`.** `grimblast`/`slurp` are wlroots-oriented. On GNOME the equivalent behaviors are Shell-internal modal UI, popup/X11 grabs, or ordinary `xdg_toplevel` windows. | Focus handling is specified by **Focus Class**, not by protocol name (§6.4). |
| **R7** | Two near-identical debounce keys (`group_register_timeout`, `auto_group_window_ms`); one lacks the `_ms` suffix. | Naming inconsistency, unclear scope. | `auto_group_window_ms` governs implicit bursts; `group_register_timeout_ms` governs explicit worker-first groups. Legacy `group_register_timeout` is accepted as an alias. |
| **R8** | A fixed 40 ms debounce on every command. | Taxes every *solitary* launch with 40 ms of dead latency. | **Adaptive fast path**: if no live sibling process exists under the same PPID, the window seals immediately (§4.2). |
| **R9** | `--priority` on grouped commands. | Undefined when members disagree. | Priority is a group-level term: first-claim, leader override, worker conflicts warned-and-ignored (§5.5). |
| **R10** | `cmd1 ; cmd2` is "strictly sequential". | True **only if** the CLI process lives until its window settles. If the CLI exited at spawn time, `;` would not serialize anything. | CLI lifetime = until `SETTLED` (not until the *application* exits). Escape hatch: `--no-wait` (§3.7). |

---

## 1. Problem Statement, Design Goals & System Invariants

### 1.1 Failure Classes Addressed

Launching several applications from scripts on Wayland/Mutter routinely produces four failure classes. Each maps to a specific mechanism in this design.

| Failure class | Manifestation | Primary mechanism | Section |
| :--- | :--- | :--- | :--- |
| **Visual flicker / relayout stutter** | Windows map out of order, resize and shift across several frames. | Pre-spawn barrier + extension-side cloak with *uncloak-on-ack*; lockstep release. | §3, §9 |
| **Focus-stealing races** | A newly mapped surface steals or cycles seat focus before adjacent tiled windows finish rendering. | Non-preemptible Settle; single-writer **Focus Authority** in the extension; 100 ms soft-yield barrier. | §3.3, §6, §9.5 |
| **Queue deadlocks** | A long multi-window transaction freezes the queue and blocks screenshot tools, clipboard pickers, launchers. | Ready-Set Dispatch (sidelining), Priority lane, **Lock Split**, `--bypass-fifo`, bounded holds. | §3.5, §5, §6 |
| **Shell burst concurrency** | `&`-chained binaries start microseconds apart and race on D-Bus. | Single-threaded coordinator, auto-grouping, single outstanding `ArmSpawn`. | §4, §8 |

### 1.2 Goals and Non-Goals

**Goals:** deterministic layout; visually simultaneous group appearance; zero side-effects from cloaked windows; bounded latency for urgent utilities; every failure mode degrades *open* (the user still gets their application).

**Non-Goals:** `spawn-at` does not manage windows after settle (no persistent tiling); it does not manipulate windows it did not claim; it does not replace the user's launcher or session manager; it does not attempt X11-style `_NET_WM_USER_TIME` forgery.

### 1.3 System Invariants

These invariants are testable properties (§12.2). Every mechanism in this document exists to preserve one or more of them.

| ID | Invariant |
| :--- | :--- |
| **I1** | **No-Spawn.** No application binary executes (`Command::spawn()`) until all terms of its transaction's contract are fulfilled (or the transaction is explicitly degraded/ejected by policy). |
| **I2** | **Single Writer.** Only the coordinator's event-loop thread mutates transaction, queue, or lock state. |
| **I3** | **Settle Exclusion.** At most one holder of `focus_execution_lock` exists at any time; a Settle section is never interleaved with another Settle or Arm section. |
| **I4** | **First-Claim.** Leadership belongs to the first frame, in linearization order, that carries `--group-size`. |
| **I5** | **Order Preservation.** Within the Priority lane, for arrival sequence $a < b$: $\text{rank}(a) < \text{rank}(b)$ always. |
| **I6** | **Bounded Waiting.** Every CLI either proceeds, degrades, or aborts within a computable bound, **except** where the user explicitly selects `none` timeouts. |
| **I7** | **Fail-Open.** No coordinator, extension, or CLI failure may leave a user application un-launched (unless `fail_closed` is selected) or a window permanently invisible. |
| **I8** | **Cloak Finiteness.** Any window cloaked by `spawn-at` is uncloaked within `cloak_max_ms`, unconditionally, even if the coordinator is dead. |
| **I9** | **Claim Finiteness.** Every claim expires within `claim_ttl_ms`; a stale claim can never capture an unrelated, later window. |
| **I10** | **Non-Blocking Core.** The coordinator event loop performs no unbounded blocking operation (no blocking I/O on client sockets, no synchronous D-Bus calls). |

---

## 2. Architecture Overview

### 2.1 Components and Trust Boundaries

```text
┌──────────────────────────── USER SESSION (uid = $UID) ────────────────────────────┐
│                                                                                   │
│  Shell / script / keybinding                                                      │
│     │  fork() × n                                                                 │
│     ▼                                                                             │
│  ┌──────────────┐   UDS: /run/user/$UID/spawn-at/coordinator.sock                 │
│  │ spawn-at CLI │◄────────────────────────────────────┐   (0600, dir 0700,        │
│  │  (Rust, 1 per│                                     │    SO_PEERCRED verified)  │
│  │   command)   │                                     ▼                           │
│  └──┬───────┬───┘                         ┌───────────────────────┐               │
│     │       │ Command::spawn()            │  Coordinator daemon   │               │
│     │       ▼                             │  (single-threaded,    │               │
│     │   ┌────────┐                        │   async, non-blocking)│               │
│     │   │ target │                        │  • Phase-1 assembly   │               │
│     │   │  app   │                        │  • Lane queues        │               │
│     │   └───┬────┘                        │  • focus_execution_   │               │
│     │       │ Wayland                     │    lock arbiter       │               │
│     │       │                             └──────────┬────────────┘               │
│     │ --bypass-fifo only:                            │ D-Bus (session bus)        │
│     │ out-of-band D-Bus call                         │ org.spawnat.Coordinator1   │
│     │ (BypassArm)                                    ▼ org.spawnat.Shell1         │
│     │                                      ┌───────────────────────┐              │
│     └─────────────────────────────────────►│ GNOME Shell extension │              │
│                                            │ • Claim table         │              │
│                                            │ • Cloak / Settle      │              │
│                                            │ • Focus Authority     │              │
│                                            └──────────┬────────────┘              │
│                                                       │ Meta / Clutter APIs       │
│                                                       ▼                           │
│                                            ┌───────────────────────┐              │
│                                            │ Mutter (compositor)   │              │
│                                            └───────────────────────┘              │
└───────────────────────────────────────────────────────────────────────────────────┘
```

**Why the CLI (not the daemon) calls `Command::spawn()`.** The user's shell supplies the working directory, environment (`PATH`, `WAYLAND_DISPLAY`, `XDG_*`, virtualenvs), and stdio. A daemon-side spawn would lose all of it. The cost — N processes must be released "simultaneously" — is solved by the barrier broadcast in §3.6, and correctness never depends on spawn simultaneity (the extension's uncloak barrier is the true synchronizer).

**Trust model.** All parties run as the same UID. The UDS enforces `SO_PEERCRED` (uid match; the peer's *kernel-reported* PID is authoritative, self-reported PID/PGID fields are cross-checked and only used for diagnostics). The extension accepts `ArmSpawn`/`Abort` only from the unique bus name that owns `org.spawnat.Coordinator1`, and accepts `BypassArm` from any same-UID peer subject to rate limiting (§6.6). Sandboxed (Flatpak) callers are mediated by the bus proxy and are **not** granted coordinator access.

### 2.2 Runtime Constraints on the Coordinator

* **Single-threaded** (`tokio` `current_thread` runtime with a `LocalSet`); all state is `!Send`, owned by one task tree. This is what makes the serialization barrier real (I2).
* **Non-blocking** (I10): all socket I/O is async with bounded buffers; D-Bus uses an async client (`zbus`) with a per-call timeout (`dbus_call_timeout_ms`); `/proc` reads are bounded, single-syscall-sized, and performed only on first-contact per burst.
* **Cancel-safety.** Every `select!` arm must be cancel-safe; timers are armed as first-class entries in a `BTreeMap<Instant, TimerEvent>` owned by the state machine, not as detached futures, so that "disband group" can deterministically cancel all of a group's timers.
* **Bounded memory.** `max_queue_depth`, `max_group_size`, and a hard 64 KiB frame ceiling bound every allocation derived from client input.

### 2.3 The Transaction Object

```rust
struct Transaction {
    id: TxnId,                    // u64, monotonic within a coordinator epoch
    epoch: u64,                   // coordinator boot nonce; stale-batch rejection (§10)
    arrival_seq: u64,             // assigned at frame-decode (linearization point)
    kind: TxnKind,                // Solitary | AutoGroup | Explicit
    scope: ScopeKey,              // hash(ctty_dev, SID, SID.start_time) unless --global
    lane: Lane,                   // Priority | Baseline
    p_assigned: u32,              // meaningful only in Priority lane (§5)
    contract: Option<Contract>,   // set once, by the first leader frame (I4)
    members: SmallVec<[Member; 4]>,
    state: TxnState,
    timers: TimerSet,             // register / group / hold / lease timers
    history: Vec<HistoryEntry>,   // for diagnostic dumps (§8.5)
}

enum TxnState {
    Registering,    // debounce window open; fences later baseline arrivals
    SealedWaiting,  // contract locked; awaiting members (sidelined unless hold)
    Ready,          // eligible for dispatch
    Arming,         // 2A: ArmSpawn in flight (lock held)
    Spawning,       // 2B: members issuing Command::spawn() (lock held)
    MapAwait,       // 2C: windows starting (lock NOT held)
    SettleQueued,   // 2D requested, awaiting lock grant
    Settling,       // 2D: critical section (lock held)
    Done,           // terminal: success
    Degraded,       // terminal: partial success (timeout / spawn failure)
    Aborted,        // terminal: policy abort / conflict poison
    Cancelled,      // terminal: leader SIGINT / ctl disband
}
```

### 2.4 IPC Wire Protocol (CLI ⇄ Coordinator)

Length-prefixed, versioned, little-endian binary frames over a `SOCK_STREAM` UDS (maximum frame 64 KiB; any oversize or malformed frame closes the connection and is logged).

| Frame | Direction | Payload (abridged) | Notes |
| :--- | :--- | :--- | :--- |
| `HELLO` | CLI → C | `proto_ver`, `argv`, parsed flags, `cwd`, self-reported `ppid/pgid/sid/ctty`, `resolved_target_id` | `arrival_seq` is assigned when this frame is fully decoded. |
| `ACK` | C → CLI | `txn_id`, `role` (`Solitary`/`Worker`/`Leader`/`AutoMember`), `epoch`; **passes the barrier fd via `SCM_RIGHTS`** | The CLI now parks on the fd (§3.6). |
| `WARN` | C → CLI | text | Non-fatal; CLI prints to `stderr` and continues. |
| `ABORT` | C → CLI | reason code, full diagnostic dump | CLI prints dump, exits with the mapped code (§7.6). |
| `GO` | C → CLI | `batch_id`, optional activation token | Normally implied by barrier-fd HUP; sent for token delivery. |
| `SPAWNED` | CLI → C | `pid` or `spawn_error(errno)` | Drives lock release (2B) and degraded handling for failed `exec`. |
| `SETTLED` | C → CLI | status (`ok`/`degraded`/`timeout`) | CLI exits (unless `--no-wait`). |
| `CANCEL` | CLI → C | reason (`SIGINT`/`SIGTERM`) | A graceful cancel; abrupt EOF is handled separately (§10.2). |

### 2.5 Transaction State Machine

```text
                                   ┌──────────────── hold/group timeout ────────────────┐
                                   ▼                                                     │
 HELLO ─► Registering ──seal──► SealedWaiting ──members complete──► Ready ──dispatch──► Arming
              │  │                    │   ▲                           ▲                   │
              │  └─leaderless seal────┼───┼───────────────────────────┘                   ▼
              │                       │   └── late leader locks contract              Spawning
              │                       │                                                   │
   conflict/poison/size-violation     │                                                   ▼
              │                       │                                               MapAwait ──lease/timeout──► (detached; lock free)
              ▼                       ▼                                                   │
           Aborted               Cancelled / Degraded (timeout flush)                     ▼
                                                                                     SettleQueued ─grant─► Settling ─► Done / Degraded
```

---

## 3. The Pre-Spawn Assembly Gate & Universal FIFO Pipeline

### 3.1 The No-Spawn Invariant

No application binary executes until every term of its transaction's contract is fulfilled (**I1**). The CLI connects, parks at the barrier, and does nothing observable to the desktop: **zero Wayland surfaces exist, no keybind grabs are taken, no audio starts, and no compositor allocation occurs** while a transaction assembles. A parked CLI costs one blocked process and one socket.

There is exactly **one** execution engine. Solitary commands, `&`-bursts, and explicit groups are all *transactions* flowing through the same pipeline; they differ only in how quickly their contract is satisfied.

```text
┌─────────────┐   ┌────────────────────────┐   ┌─────────────┐   ┌───────────────────────────────────────────┐
│ CLI check-in│──►│ PHASE 1: ASSEMBLY      │──►│ READY SET   │──►│ PHASE 2: DISPATCH & FOCUS LOCK            │
│ (HELLO)     │   │ (preemptible)          │   │ lane,P,seq  │   │ 2A Arm → 2B Spawn → 2C Map-Await → 2D Settle│
└─────────────┘   │ barrier parking,       │   └─────────────┘   │ (non-preemptible critical sections)       │
                  │ debounce, contract     │                     └───────────────────────────────────────────┘
                  └────────────────────────┘
```

### 3.2 Phase 1 — Assembly (Preemptible)

Phase 1 comprises: socket handshake → barrier parking → contract checks → debounce windows (§4). It touches no compositor state, so it is **safely preemptible**: a higher-ranked transaction can be dispatched past any transaction that is still assembling (subject to the fence/hold rules in §3.5). The five steps are:

1. **Registration check-in.** Every invocation (`spawn-at spawn …`, with or without `--group`) connects to `coordinator.sock` and sends `HELLO`. Flag-authority and incompatibility checks (§7.3) have *already* run client-side, so malformed invocations never reach the coordinator.
2. **Pre-Spawn Barrier.** The coordinator replies `ACK` carrying the barrier fd. The CLI parks on it. Nothing else happens in the desktop.
3. **Contract resolution & leadership assignment.** The coordinator evaluates the frame against the scope's active transaction: leader authority (first-claim, §8), size completeness, auto-group eligibility (§4.2), priority term (§5.5).
4. **Seal.** The transaction becomes **Ready** when (a) the leader's declared $N$ members have arrived, (b) a debounce window closes (auto/leaderless), or (c) `group_timeout` forces a degraded flush.
5. **Hand-off to Phase 2** by dispatch from the Ready Set (§3.5).

### 3.3 Phase 2 — Dispatch & The Focus Lock (Non-Preemptible)

Phase 2 is governed by the atomic **`focus_execution_lock`**, arbitrated by the coordinator and mirrored by the extension's own *Batch Mutex* (defense in depth, I3).

**[R3] Lock Split.** Phase 2 has four sub-stages. The lock covers two *critical sections*; the slow, non-deterministic stage (waiting for applications to start and map) runs **outside** the lock.

| Stage | Name | Lock held? | Typical duration | What happens |
| :--- | :--- | :---: | :--- | :--- |
| **2A** | **Atomic Arm** | **Yes** | 1–3 ms | Coordinator sends **one** `ArmSpawn(batch)` D-Bus call (all entries, all geometry, claim identities). Extension installs claims and **acks**. Single outstanding call; never one call per window or per property. |
| **2B** | **Lockstep Spawn** | **Yes** | < 2 ms | Coordinator closes the barrier write-end; all CLIs wake and call `Command::spawn()` and report `SPAWNED`. Lock drops when all members report (or `spawn_report_timeout_ms`). |
| **2C** | **Map-Await** | **No** | 10 ms – seconds | Applications start. Extension matches newly created windows to claims (§9.1) and keeps them **cloaked** (opacity 0, input-inert). Other transactions may arm/spawn/settle meanwhile. Bounded by `map_timeout_ms`. |
| **2D** | **Settle** | **Yes** | **~15–30 ms** | Once **all** members have mapped (or the batch is degraded), the extension calls `RequestSettle(batch_id)`; on **grant** it runs: configure-ack barrier → sizing clamp → anchor + post-clamp margin re-evaluation → **uncloak ramp $0 \to 255$** → focus handoff. Lock drops on `BatchSettled`. |

**Why the split matters.** If the lock spanned 2C, an Electron application taking 1.8 s to map would block every screenshot tool, clipboard picker, and launcher for 1.8 s — exactly the *Queue Deadlock* failure class (§1.1). With the split, the longest any other transaction can be blocked by a Phase 2 holder is:

$$W_{\max}^{\text{block}} \;=\; \max\big(T_{2A}+T_{2B},\; T_{2D}\big) \;\le\; \texttt{spawn\_report\_timeout\_ms} + \texttt{settle\_watchdog\_ms}$$

and the **typical** blocking time is the 15–30 ms Settle of the transaction currently holding the lock.

**Settle re-entry.** A transaction that leaves 2C holds a *Settle ticket* whose sort key is its **original** $(\text{lane},\, P_{\text{assigned}},\, \text{arrival\_seq})$ (§3.5). Tickets are served before undispatched transactions of the same or lower rank — already-spawned, cloaked windows must not wait behind new work. `RequestSettle` is an async-reply method call: if the lock is free the reply is immediate (one round trip on the uncontended path).

**Non-preemption rule.** Nothing — not even Priority 0, not `--bypass-fifo` focus requests (§6) — interrupts a live 2A/2B/2D section. Interruption mid-clamp causes visible flicker and compositor-level races. Everything else waits for the lock.

```text
 CLI(s)           Coordinator                 Extension (Shell)               Mutter / Apps
   │─── HELLO ───────►│                              │                            │
   │◄── ACK(+fd) ─────│  Phase 1: parked             │                            │
   │                  │ [contract satisfied → Ready] │                            │
   │                  │══ acquire focus_execution_lock ══                         │
   │                  │─── ArmSpawn(batch) ─────────►│  install claims            │      ┐
   │                  │◄── Armed (ack) ──────────────│                            │      │ 2A
   │◄══ barrier HUP (GO) ═══│                         │                            │      ┘
   │─── Command::spawn() ─────────────────────────────────────────────────────────►│      ┐ 2B
   │─── SPAWNED ─────►│                              │                            │      │
   │                  │══ release lock ══            │                            │      ┘
   │                  │                              │◄─ window-created ──────────│      ┐
   │                  │                              │   match claim; CLOAK       │      │ 2C
   │                  │                              │◄─ (all N members mapped) ──│      ┘ (lock free)
   │                  │◄── RequestSettle ────────────│                            │
   │                  │══ grant (acquire lock) ══════►                            │
   │                  │                              │ ack-configure → clamp →    │      ┐
   │                  │                              │ anchor → uncloak 0→255 →   │      │ 2D
   │                  │                              │ focus handoff              │      │
   │                  │◄── BatchSettled ─────────────│                            │      ┘
   │                  │══ release lock ══            │                            │
   │◄── SETTLED ──────│                              │                            │
   ▼  CLI exits (0)
```

### 3.4 Command Routing Flow

```text
                      HELLO arrives (linearized: arrival_seq = n)
                                      │
               ┌──────────────────────┴───────────────────────┐
        --bypass-fifo ?                                       no
               │ yes                                           │
   (never reaches the coordinator:             has --group ? ──┴── no ──► auto-group eligible? (§4.2)
    CLI → extension out-of-band, §6)                │ yes                         │ yes            │ no
                                                    ▼                             ▼                ▼
                                       Explicit transaction            Join/open Auto-Group    Solitary (N = 1)
                                       (leader-first / worker-first /   (Registering window)    (immediate seal;
                                        leaderless; §7.5)                                         fast path §4.2)
                                                    └───────────────┬─────────────┘                │
                                                                    ▼                              │
                                          Lane := --priority ? Priority : Baseline ◄───────────────┘
                                                                    │
                                                                    ▼
                                                    Phase 1 → Ready Set → Phase 2
```

### 3.5 Ready-Set Dispatch (Unifying FIFO, Sidelining, Holds and Priorities)

**[R2]** FIFO order applies to the **Ready Set**. The dispatcher runs whenever the lock drops or a transaction becomes Ready, and picks the Ready entry (or Settle ticket) minimizing the comparator:

$$\text{key}(t) \;=\; \big(\;\text{lane}(t),\;\; P_{\text{assigned}}(t),\;\; \text{arrival\_seq}(t)\;\big) \quad\text{(lexicographic, ascending)}$$

with $\text{lane}(\text{Priority}) = 0 < \text{lane}(\text{Baseline}) = 1$ and $P_{\text{assigned}} = 0$ for all baseline entries. Consequently, **within the Baseline lane the order is pure arrival FIFO**.

Phase-1 state determines whether an *unready* transaction can obstruct others:

| Unready transaction state | Effect on **later-arriving Baseline** transactions | Effect on **Priority-lane** transactions | Rationale |
| :--- | :--- | :--- | :--- |
| **Registering** (debounce window open) | **Fences** them until sealed (bounded by `auto_group_max_window_ms` / `group_register_timeout_ms`) | None (preempts) | Window is short; preserves arrival order of near-simultaneous commands. |
| **Sealed-Waiting**, `--sideline` (default) | None — they dispatch past it | None | The contract is locked; the group is idle and holds no compositor resource. |
| **Sealed-Waiting**, `--hold` / `--no-sideline` | **Block** until the group is Ready or hold expiry (§7.4) | None (idle bypass); a held *Priority-lane* group blocks only transactions ranked after it | Explicit user intent: "do not interleave anything until my group appears." |
| **Settling / Arming / Spawning** (lock held) | Wait for the lock | Wait for the lock (non-preemptible) | I3. |
| **MapAwait** (lock free) | None (subject to the same-class interlock, §9.1) | None | R3. |

**Ordering guarantees and their explicit escape hatches.** The user's requirement is that running one command after another stays deterministic. The guarantees, and the *only* ways to depart from them, are:

| # | Guarantee | Only broken by |
| :--- | :--- | :--- |
| **G1** | `cmd1 ; cmd2` — `cmd2` always starts after `cmd1`'s window has settled (CLI lifetime = until `SETTLED`, **R10**). | `--no-wait` on `cmd1`. |
| **G2** | `cmd1 & cmd2 &` (one burst) appear **atomically**, on the same frame. | `--no-auto-group` / `auto_group_enabled = false`. |
| **G3** | A later command (`cmd3`) arriving after a burst's debounce window **never** joins it and always dispatches after it. | `--priority`, a `--sideline`d group still assembling, `--bypass-fifo`. |
| **G4** | Between two Baseline transactions, arrival order = dispatch order. | A Sealed-Waiting sidelined group (it is not Ready); `--priority`. |
| **G5** | A transaction already in Phase 2 is never disturbed. | Nothing. |

### 3.6 Barrier Broadcast: One Syscall, N Wakeups

"Lockstep spawn" must not degrade into a sequential `write()` to N sockets (each wake-up skewed by a syscall plus scheduler latency). The coordinator therefore uses the **pipe-HUP broadcast idiom**:

1. At transaction creation the coordinator opens a pipe `(r, w)` and retains `w`.
2. Every member receives `r` (duplicated via `SCM_RIGHTS`) in its `ACK`.
3. Each CLI `poll()`s `r` for `POLLHUP`.
4. At release the coordinator **closes `w`** — a single syscall that wakes every waiter at once.

Spawn skew is thereafter bounded only by the kernel scheduler. **Correctness never depends on it**: the extension keeps all members cloaked until the last claim is fulfilled, so even a 300 ms skew between `alacritty` and `code` produces a single-frame appearance.

### 3.7 CLI Lifecycle

| Stage | CLI behavior |
| :--- | :--- |
| Parse & validate | Flag-authority and incompatibility checks; exit code `2` on violation (no IPC). |
| Connect | `connect()` with `coordinator_connect_timeout_ms`; on failure → degraded mode per `coordinator_failure_policy` (§10). |
| Park | Block on barrier fd; `SIGINT`/`SIGTERM` handlers send `CANCEL`. |
| Spawn | `Command::spawn()` with stdio detached, new process group; reports `SPAWNED`. |
| Wait-for-settle | Block until `SETTLED` (default). The application keeps running; **the CLI exits as soon as the window has settled, not when the application exits.** |
| Exit | `0` on success; non-zero mapping in §7.6. `--no-wait` exits immediately after `SPAWNED` (used by keybindings that must return instantly). |

---

## 4. Execution Modes & Shell Concurrency Semantics

How a command travels through Phase 1 depends entirely on how the shell invoked it.

### 4.1 Sequential Execution (`;` and Solitary Spawns)

Every ungrouped command is an **implicit transaction of size 1** — strictly serialized through the pipeline.

```bash
spawn-at spawn --anchor left alacritty ; spawn-at spawn --anchor right code
```

* **Mechanism.** The shell runs `alacritty`'s `spawn-at` and **waits for it to exit** before forking the second. Because the CLI exits only on `SETTLED` (R10), `code` is not even *started* until the first window has settled.
* **Coordinator view.** At any instant exactly one `spawn-at` process exists. There is no concurrency to merge, therefore **no auto-grouping can occur** — the burst key's concurrent-presence condition (§4.2) is structurally unsatisfiable under `;`.
* **Solitary lifecycle.** A size-1 transaction has no members to wait for, so it seals immediately (after the adaptive probe, §4.2): `HELLO → ACK → Ready → 2A → 2B → 2C → 2D → SETTLED`. It never registers into a group table, but it **does** respect the fence and hold rules of §3.5, and it does **not** bypass the pipeline.

**Interaction of a solitary command with in-flight transactions** (replacing the earlier "solitary bypasses groups" model, which destroyed ordering determinism):

| In-flight state | Solitary (Baseline) command behavior |
| :--- | :--- |
| A sidelined group is Sealed-Waiting | Dispatches immediately (if the lock is free) — the group is not Ready and not occupying the head. The group's layout is unaffected: its geometry is computed at *its own* Settle. |
| A `--hold` group is Sealed-Waiting | **Waits** until the group is Ready and dispatched, or hold expiry. Wakes in arrival order. |
| A burst's debounce window is open (Registering) | **Fenced** for at most the window (≤ 40 ms typical, ≤ `auto_group_max_window_ms` hard cap), then dispatches behind the burst. |
| A transaction is in 2A/2B/2D | Waits for the lock (typically 15–30 ms). |
| A transaction is in 2C (MapAwait) | Proceeds; subject to the same-class interlock (§9.1) only if both target a PID-opaque class. |

### 4.2 Concurrent Execution (`&` Auto-Grouping)

```bash
spawn-at spawn --anchor left alacritty & spawn-at spawn --anchor right code &
```

The shell forks both CLIs within microseconds. Both reach the socket within roughly 2–8 ms.

**Burst key.** **[R1]**

$$\text{burst\_key} \;=\; (\text{uid},\; \text{SID},\; \text{PPID}) \qquad\text{with}\qquad \text{PPID} \notin \{\text{compositor},\ \text{systemd --user},\ 1\}$$

* **PGID is *not* part of the key.** Interactive shells with job control place each background job in its own process group, so `cmd1 & cmd2 &` has two PGIDs. Non-interactive scripts share one. PGID is logged for diagnostics only.
* **Identity comes from the kernel.** PID is taken from `SO_PEERCRED`; `ppid`/`sid`/`start_time` are read from `/proc/<pid>/stat` immediately after accept, and the peer is pinned with `pidfd_open()` so PID reuse cannot corrupt the key.
* **Keybinding guard.** Processes launched directly by the compositor's keybinding handler all share `PPID = gnome-shell`. Two keypresses 30 ms apart must never be merged, so the compositor PID (resolved once via `GetConnectionUnixProcessID` on `org.gnome.Shell`) and `systemd --user` are excluded as burst parents. Shell scripts invoked by a keybinding are fine: their *children's* PPID is the script's shell.
* **Advisory signal (never a gate).** `tcgetpgrp(STDIN) != getpgrp()` indicates a background job. It is absent without a TTY and unreliable in scripts, so it only corroborates.

**Auto-group eligibility.** A frame joins an open Auto-Group iff **all** hold:

1. `auto_group_enabled = true` and the frame has no `--no-auto-group` (alias `--solo`);
2. same `burst_key`;
3. the frame has **no** `--group` (explicit groups never merge with implicit ones) and no `--bypass-fifo`;
4. identical `--priority` value (absent = absent) — differing lanes cannot share a transaction (R9);
5. the open window is still **Registering**, and the group is below `auto_group_max_members`.

Because a `;`-sequenced predecessor has already exited, rule 5 is satisfied only by genuinely concurrent siblings — the **Concurrent Presence Invariant**.

**The rolling debounce window.** Timer: `auto_group_window_ms` (default **40 ms**; the `group_register_timeout` of earlier drafts, **R7**).

* The first frame opens the window with deadline $d = t_0 + \texttt{auto\_group\_window\_ms}$.
* Each *merged* arrival at $t_i$ **rolls** the deadline: $d \leftarrow \min\big(t_i + \texttt{auto\_group\_window\_ms},\; t_0 + \texttt{auto\_group\_max\_window\_ms}\big)$. The hard cap (default 250 ms) means a runaway loop of `&` spawns can never extend a fence indefinitely.
* At $d$ the group **seals at $N = |\text{members}|$** and becomes Ready. All members are held at the barrier until then and are released together.

**[R8] Adaptive fast path (no latency tax on solitary spawns).** On the first frame of a burst key the coordinator enumerates `/proc/<PPID>/task/*/children` and counts **sibling candidates**: live children (other than the peer) whose `/proc/<pid>/exe` is the `spawn-at` binary, *or* the same binary as the parent shell (a forked-but-not-yet-exec'd job). If the count is zero, the window seals **immediately** (zero added latency). If siblings exist, the full rolling window runs. Probe failure (e.g., `CONFIG_PROC_CHILDREN` absent, permission error) falls back to the fixed window. The debounce remains the safety net; the probe is an optimization.

```text
 t=0 ms    alacritty  HELLO  (burst_key K, probe: 1 sibling candidate → open window, d = 40)
 t=4 ms    code       HELLO  (same K, eligible → MERGE, d = min(44, 250) = 44)
 t=44 ms   timer fires → seal N = 2 → Ready → Phase 2   ← both appear on the same frame
 ─────────────────────────────────────────────────────────────────────────────────────
 t=130 ms  apple      HELLO  (user pressed Enter again; window long closed)
                       → no open window for K → new Solitary transaction
                       → dispatches AFTER the burst (G3); never swallowed
```

**Why `cmd2` (`apple`) can never be swallowed by `cmd1`'s burst.** `apple` typed at the next prompt has the *same* burst key (same interactive shell, same SID/PPID), and the burst's CLIs may well still be alive waiting for `SETTLED`. Key equality alone therefore proves nothing; the barrier is **rule 5**: a frame can only merge into a transaction that is still **Registering**. By the time `apple` connects, the burst's window has sealed and the transaction is Ready, Spawning, or beyond, so `apple` opens a fresh Solitary transaction. Arrival-order dispatch (G3) then queues it **behind** the burst — never inside it, never ahead of it.

### 4.3 Explicit Multi-Window Groups (`--group`)

```bash
spawn-at spawn --group dev --group-size 2 alacritty &
spawn-at spawn --group dev code &
```

Contracts are defined explicitly via CLI flags; **leader election** follows the first-claim rule (§8). Auto-group heuristics are bypassed in favor of the declarative contract. Lifecycle shapes (leader-first, worker-first, leaderless, strict hold) are tabulated in §7.5; scoping is by default per-TTY/session (`dev` in two terminals never collide) unless `--global` is given (§7.1).

---

## 5. Inverted Priority Preemption Queue with Integer Compaction

System utilities — clipboard managers, application launchers — must not queue behind a heavy multi-window development layout that is assembling. The Priority lane exists for them, **while still passing through the coordinator** so they cannot corrupt layout calculations or race the focus lock (contrast `--bypass-fifo`, §6).

### 5.1 Inverted Semantics

Priority follows the POSIX `nice` convention: **a lower number runs earlier.**

* **Priority `0`** — maximum urgency.
* `--priority <K>` places the transaction in the **Priority lane**.
* **Default** (no flag) places it in the **Baseline lane**, which is dispatched only when the Priority lane has no Ready entry (subject to the starvation guard, §5.6).

### 5.2 Monotonic Relative Insertion

A priority command **does not override in-flight priority tasks.** Its offset is applied relative to the lane's current ceiling:

$$P_{\max} \;=\; \max\big(\{\,P_i \mid i \in \text{PriorityQueue}\,\}\big), \qquad P_{\max} = 0 \ \text{if the queue is empty}$$

$$P_{\text{assigned}} \;=\; P_{\max} + K$$

So a task requesting `--priority 5` while `rofi` sits at `10` is assigned $10 + 5 = 15$: it queues **behind** `rofi`. Existing priority work is never reordered or starved by later arrivals.

### 5.3 Order-Preservation Property (Formal Statement) — **[R5]**

For priority-lane tasks $a, b$ with $\text{arrival\_seq}(a) < \text{arrival\_seq}(b)$:

$$P_{\text{assigned}}(b) \;=\; P_{\max}^{(b)} + K_b \;\ge\; P_{\max}^{(b)} \;\ge\; P_{\text{assigned}}(a)$$

with ties resolved by $\text{arrival\_seq}$, therefore $\text{rank}(a) < \text{rank}(b)$ for every pair (**I5**). This has two direct consequences, stated plainly so that nobody expects otherwise:

1. **Starvation within the lane is impossible** — the guarantee the relative-insertion scheme was designed to deliver.
2. **$K$ cannot make a later task overtake an earlier one.** The observable effect of `--priority <K>` is (a) *entry into the Priority lane* (ahead of all Baseline traffic that is idle in Phase 1) and (b) *spacing* between entries. Users wanting a task to jump other priority tasks have no such mechanism by design; the escape hatch is `--bypass-fifo`.

### 5.4 Consecutive Compaction (Integer Overflow Prevention)

$P_{\text{assigned}} = P_{\max} + K$ grows monotonically, so over multi-month uptime values would drift upward. The coordinator **continuously re-indexes** the active priority tasks to $[0, 1, \dots, N-1]$.

**Trigger** (`compaction_mode`): *eager* (default) — after **every** enqueue and dequeue; or *threshold* — when $P_{\max} > \texttt{compaction\_threshold}$.

**Algorithm** ($O(N\log N)$, $N \le$ `max_queue_depth`):

```rust
fn compact(queue: &mut Vec<PriorityEntry>, p_max: &mut u32) {
    // 1. Stable sort by (current P_assigned, arrival_seq): relative order incl. ties preserved.
    queue.sort_by_key(|e| (e.p_assigned, e.arrival_seq));
    // 2. Count & re-index to consecutive ranks 0..N-1.
    for (rank, e) in queue.iter_mut().enumerate() {
        e.p_assigned = rank as u32;
    }
    // 3. Reset the ceiling to N-1 (0 if empty).
    *p_max = queue.len().saturating_sub(1) as u32;
}
```

Worked example:

$$[\text{Task}_A(P{=}15),\ \text{Task}_B(P{=}25),\ \text{Task}_C(P{=}80)] \;\xrightarrow{\text{compact}}\; [A \to 0,\ B \to 1,\ C \to 2], \qquad P_{\max} = N - 1 = 2$$

**Overflow bound.** With `priority_max_k` validated at the CLI and eager compaction, every stored $P \le (N_{\max}-1) + K_{\max}$ (default $255 + 1000 = 1255$), far below `u32::MAX`; arithmetic additionally uses `saturating_add`. Compaction is a pure re-labelling of a total order and never changes it (I5).

### 5.5 Priority on Groups

Priority is a **group-level contract term** (R9):

* The **first frame in the transaction carrying `--priority`** sets it (first-claim).
* A **leader's** `--priority`, if present and different, supersedes it (leader authority); $P_{\text{assigned}}$ is recomputed with the transaction's **original** `arrival_seq` preserved.
* A **worker's** differing `--priority` is **warned and ignored** (non-fatal — priority cannot corrupt layout, so it does not warrant the `abort_group` machinery).
* Auto-group members must have identical priority values to merge (§4.2, rule 4).
* A priority group becomes Ready only when assembled; until then it is idle in Phase 1 and participates in fence/hold rules like any other transaction.

```bash
# Two utilities that must appear together, ahead of normal traffic:
spawn-at spawn --group quick-tools --group-size 2 --priority 0 tool1 &
spawn-at spawn --group quick-tools tool2 &
```

### 5.6 Starvation Guard (Lane vs. Lane)

Monotonic insertion protects the lane's *internal* order, but an unending stream of Priority arrivals could starve the Baseline lane. After `max_consecutive_priority` (default **8**) consecutive Priority dispatches, the dispatcher serves **one** Ready Baseline entry (if any), then resumes. A misbehaving script spamming `--priority 0` therefore degrades gracefully instead of freezing normal launches.

### 5.7 Phase Interaction

```text
 Incoming Priority command (offset +K)
                │
                ▼
   Is a transaction holding focus_execution_lock (2A / 2B / 2D)?
        ├── YES ──► Park at barrier; dispatched at lock drop (≈15–30 ms typical;
        │           bounded by §3.3). Never interrupts the critical section.
        └── NO  ──► Evaluate Phase 1 state of others:
                      • idle Registering / Sealed-Waiting transactions are BYPASSED
                        (Phase-1 preemption — they have touched no compositor state)
                      • P_assigned = P_max + K; insert; run compaction → ranks 0..N-1
                      • becomes Ready (after its own seal) and wins the next dispatch
```

* **Phase 1 preemption:** priority work jumps ahead of idle assembly — including `--hold` groups, since a held group in Phase 1 holds no focus.
* **Phase 2 blocking:** priority work waits for the Settle (or Arm/Spawn) section in progress.

**Worked timeline** (all values illustrative):

| $t$ (ms) | Event | Lane state after event | Dispatcher action |
| ---: | :--- | :--- | :--- |
| 0 | Group `dev` (Baseline, `--group-size 3 --hold`) registers; 1 of 3 members present | Baseline: `dev` (Sealed-Waiting, hold) | Nothing Ready |
| 5 | `copyq show` arrives, `--priority 10` | Priority: `copyq` $P{=}10 \to$ compacted $0$ | Idle-bypasses the held group; `copyq` becomes Ready → dispatch (lock free) |
| 7 | `rofi`, `--priority 20`, arrives while `copyq` is in 2D | Priority: `copyq`(0, Settling), `rofi` $P = 0 + 20 = 20 \to$ compacted $1$ | `rofi` waits for lock |
| 28 | `copyq` `BatchSettled` | Priority: `rofi`(0) | `rofi` dispatched |
| 52 | `rofi` settled | Priority empty; Baseline: `dev` still Sealed-Waiting | Group resumes waiting; layout math untouched |
| 700 | Members 2, 3 of `dev` arrive | `dev` Ready | Dispatched; appears atomically |

(`rofi` follows `copyq` by I5, regardless of the offsets requested.)

---

## 6. Out-of-Band Execution (`--bypass-fifo`) & Wayland Focus Arbitration

### 6.1 Purpose

For urgent transient utilities — screenshot tools, clipboard pickers, launchers (`grimblast`, `slurp`, `copyq`, `rofi` are the canonical examples; see the platform note in §6.4) — waiting even 15–30 ms for a Settle, let alone behind a held group, is unacceptable. `--bypass-fifo` (aliases `--instant`, `--raw`) **skips the coordinator socket, barriers, and FIFO queues entirely.**

```bash
spawn-at spawn --bypass-fifo --anchor center rofi -show drun
spawn-at spawn --instant --anchor top-right copyq show
```

### 6.2 Execution Semantics

1. **Zero coordinator contention.** No connection to `coordinator.sock`; no group registration; no assembly gate; no `focus_execution_lock`; any active `--hold` is ignored.
2. **Immediate spawn.** `Command::spawn()` on tick 0, after a single out-of-band D-Bus message.
3. **Out-of-band D-Bus dispatch.** The CLI calls `org.spawnat.Shell1.BypassArm(entry)` directly. Messages from a single D-Bus connection are delivered in order, so `BypassArm` is guaranteed to reach the extension **before** the spawned process can map a window.
4. **Express claim.** The extension records the entry in the **Express Claim Table** (separate from queued batches, §9.1), places the window as soon as Mutter maps it (geometry clamp + anchor + uncloak, no group barrier), and raises the **soft-yield flag** (§6.5).
5. **Fail-open.** If the extension is absent, the CLI prints a one-line warning and spawns raw. A bypass utility is never blocked by `spawn-at` infrastructure.

### 6.3 Incompatibility Invariants

`--bypass-fifo` is rejected client-side (exit `2`, before any IPC) in combination with any flag that presupposes the coordinator:

```text
Error: Incompatible flags.
'--bypass-fifo' cannot be used with multi-window group transactions ('--group', '--group-size'),
queue controls ('--priority', '--sideline', '--no-sideline', '--hold', '--group-timeout',
'--hold-timeout'), or '--global'.
```

(`--priority` is excluded because the Priority lane *is* the coordinated alternative: **use `--priority 0` when you want urgency without leaving the pipeline; use `--bypass-fifo` when you must not wait for even a Settle.**)

| Mode | Barrier wait | Queue position | Blocked by `--hold` groups? | Primary use |
| :--- | :--- | :--- | :--- | :--- |
| **Group member** (`--group`) | Until contract satisfied | Batched with its members | N/A | Multi-window layouts |
| **Solitary / Auto-Group** (default) | Fast path ≈ 0 ms; ≤ 40 ms if siblings | Baseline lane, arrival FIFO | **Yes** (fence/hold) | Normal launches, scripts |
| **Priority** (`--priority K`) | Own seal only | Priority lane; ahead of idle Phase 1 | **No** (idle bypass) | Launchers, pickers needing ordering safety |
| **Out-of-band** (`--bypass-fifo`) | **None** | **None** | **Never** | Screenshot/OSD/clipboard overlays |

### 6.4 Wayland Focus Collision Handling

The question: a `--bypass-fifo` utility starts at the exact moment an in-flight group is in its final uncloak/focus step. **Geometry and focus are independent systems.** Window rectangles come from monitor work areas and the extension's anchor math; keyboard focus is an input-routing decision. A newly appearing window does not move or resize existing ones (Mutter does not re-tile on map), so the group's layout is never invalidated by a bypass window. Only *focus* needs arbitration.

> **Platform reality check — Mutter vs. layer-shell. [R6]** The `zwlr_layer_shell_v1` protocol that `slurp`/`grim`-family tools and `rofi-wayland` rely on is a **wlroots-ecosystem** extension; **Mutter does not implement it.** On GNOME the behaviors that earlier drafts attributed to "Layer Shell" are provided by three different mechanisms, which is why this design classifies utilities by **Focus Class** rather than by protocol name. The tool names in §6.1 are retained as *categories of utility* (transient overlay with immediate input needs); on a GNOME session the practical equivalents are GNOME's own Shell screenshot UI, portal-mediated tools, X11 clients under XWayland, and ordinary `xdg_toplevel` windows.

| Focus Class | Examples on GNOME/Mutter | How focus is obtained | Interaction with an in-flight group | Soft-yield needed? |
| :--- | :--- | :--- | :--- | :---: |
| **A — Shell-internal modal UI** | GNOME's built-in screenshot/screencast UI, overview, run dialog, `xdg-desktop-portal-gnome` pickers | `Main.pushModal` / `Meta.ModalTracker` **compositor input grab** — outranks window focus entirely | Group completes geometry underneath; input belongs to the grab. | No |
| **B — Compositor-mediated grabs** | `xdg_popup` grabs (menus), XWayland clients using keyboard/pointer grabs (e.g., X11 launchers), screen-capture overlays under XWayland | Protocol-level grab honoured by Mutter | Grab holder keeps input; the group's `window.activate()` would otherwise *break* the grab, so it must be suppressed. | **Yes** |
| **C — Ordinary `xdg_toplevel`** | `copyq`, Wayland-native launchers/pickers, GUI popups | Mutter **Focus Stealing Prevention (FSP)**: timestamp-based `user_time` / `xdg-activation-v1` token serial. A request with a *stale* timestamp becomes "demands attention", not focus. | The group's `window.activate(global.get_current_time())` carries a **fresh** timestamp and can win focus *after* the utility opened. | **Yes** |

**Why timestamps alone are not enough.** FSP outcomes depend on which actor holds the freshest user-time and on whether the launcher passed an activation token. An extension-issued `activate(now)` is always "fresh", so a group's **delayed** focus call can reorder focus 10 ms after the user's urgent window appeared. The design therefore does **not** rely on FSP to pick the right winner — it removes the contender (§6.5).

### 6.5 The 100 ms Soft-Yield Barrier

Implemented at the **extension** level. When `BypassArm` arrives:

```text
bypass_active_until = max(bypass_active_until, now_monotonic + bypass_yield_ms)     # default 100 ms
                      capped at  first_bypass_time + bypass_yield_max_ms             # default 1000 ms
```

While `now < bypass_active_until`, the extension's **Focus Authority** (§9.5) **suppresses `window.activate()` / raise / focus requests issued on behalf of in-flight groups** — including delayed intra-group focus cycling. **Geometry, clamping, anchoring and opacity are unaffected**; the group still snaps into place and becomes visible, but its leader appears as *inactive* while the urgent window keeps keyboard focus undisturbed.

**Deferred Focus Restoration.** A suppressed focus request is not discarded blindly; it is stored as `deferred_focus = (batch_id, window)`. When the barrier expires:

* If the bypass window **still holds focus** (the user is interacting with it) → the deferred request is **dropped** (never steal back).
* If the bypass window **no longer exists or no longer has focus** (e.g., `rofi` dismissed within 40 ms, or the utility failed to map) → focus is **restored** to the deferred target, so the desktop never ends up with no focused window. (`bypass_restore_deferred_focus = true`.)

```text
 t=0    ms  Group enters 2D: ack-configure, clamp, anchor, start uncloak ramp.
 t=5    ms  User hits hotkey → spawn-at --bypass-fifo ... :
              CLI → BypassArm  (ext sets bypass_active_until = t+105)   ← precedes spawn
              CLI → Command::spawn()
 t=9    ms  Bypass window maps → Express Claim fulfilled → placed + uncloaked → ext activates it
 t=12   ms  Group script fires its delayed focus-switch to Window 2
              → Focus Authority: now < bypass_active_until → SUPPRESSED (deferred_focus := W2)
 t=22   ms  Group's uncloak ramp completes (0 → 255). Geometry final. Group visible, inactive.
 t=105  ms  Barrier expires → bypass window still focused → deferred_focus dropped.
 ──────────────────────────────────────────────────────────────────────────────────────
 Outcome: layout intact, bypass utility owns the keyboard, no flicker, no focus ping-pong.
```

**Properties.** The flag uses `GLib.get_monotonic_time()` (wall-clock jumps cannot extend or shrink it). Repeated bypass launches **roll** the deadline up to the cap. The barrier suppresses only focus-class operations; a group's `BatchSettled` is still emitted so its CLIs exit normally.

### 6.6 Express Path Hardening

| Hazard | Mitigation |
| :--- | :--- |
| **Key-repeat / script flood** (a held hotkey spawning hundreds of `rofi`) | Extension token bucket per canonical class (`bypass_rate_limit_per_sec`, default 10); excess `BypassArm` returns `Throttled`, CLI prints a warning and **still spawns raw** (the app launches; only placement is skipped). |
| **Class collision with a queued batch** (a bypass `alacritty` while a group armed `alacritty`) | Express claims match by **PID/token first**; class-only matching is permitted only when no queued batch holds an outstanding claim for that class (§9.1). |
| **No coordinator awareness** | The coordinator never sees bypass traffic; consistency is the extension's job via its Batch Mutex, which orders *settle sections* of express and queued windows so that two clamps never run interleaved. Bypass geometry is independent of group geometry, so the interleaving is safe. |
| **Extension absent** | Raw spawn; one warning; exit `0`. |

---

## 7. CLI Flag Taxonomy & Authority Hierarchy

### 7.1 General Flags

| Flag | Meaning |
| :--- | :--- |
| `--group <NAME>` | Assign the window to a named transaction group. **Scoped to the current PTY/Session/PGID context by default** (`dev:pts-2` vs `dev:pts-4`). |
| `--global` | Elevate the group's namespace to **session-wide**, enabling cross-terminal coordination (e.g., a background daemon's window with a shell-launched window). Requires `--group`. |
| `--priority <K>` | Enter the Priority lane with relative offset $K$ (lower = earlier, POSIX-`nice` convention). $K \in [0, \texttt{priority\_max\_k}]$. |
| `--bypass-fifo` (aliases `--instant`, `--raw`) | Out-of-band execution (§6). |
| `--no-auto-group` (alias `--solo`) | Opt this command out of `&`-burst merging (§4.2). |
| `--no-wait` | Exit as soon as `Command::spawn()` has been issued instead of waiting for `SETTLED` (§3.7). Forfeits guarantee G1 for this command. |

### 7.2 Leader Authority Flags (Exclusive to `--group-size`)

Only the process carrying **`--group-size`** holds **Leader Authority**. It defines the contract. **Workers attempting any leader flag are rejected** with a fatal flag-authority violation (client-side, exit `2`).

| Flag | Meaning | Default |
| :--- | :--- | :--- |
| `--group-size <N>` | Declares the expected group size $N$ and **claims leadership**. $1 \le N \le \texttt{max\_group\_size}$; requires `--group`. | — |
| `--sideline` | Yield the system-wide spawn lock once the contract is locked; unrelated commands may dispatch while the group waits for members. | **default** |
| `--no-sideline` (alias `--hold`) | Hold the global queue: unrelated Baseline commands queue behind this group until it assembles (or hold expiry). | off |
| `--group-timeout <ms\|none>` | Maximum time to wait for missing members before a **degraded flush**. | `group_timeout_ms` |
| `--hold-timeout <ms\|none>` | Maximum time to freeze global spawns under `--hold`. | `hold_timeout_ms` |

**Infinite-hold deadlock warning.** `--hold-timeout none` prints, to `stderr`, at parse time and again in the coordinator's `ACK`:

```text
Warning: Infinite hold timeout requested. If group members fail to arrive, the desktop spawn queue will deadlock.
         Recovery: Ctrl+C this process, or run:  spawn-at ctl disband <group>
```

### 7.3 Validation & Incompatibility Matrix (client-side, pre-IPC)

| Invocation | Result |
| :--- | :--- |
| Leader flag (`--sideline`, `--hold`, `--group-timeout`, `--hold-timeout`) **without** `--group-size` | **Fatal**, exit `2`: *flag authority violation*. |
| `--group-size` without `--group` | Fatal, exit `2`: *group-size requires --group*. |
| `--global` without `--group` | Fatal, exit `2`. |
| `--group-size 0`, or $> $ `max_group_size`, or non-numeric | Fatal, exit `2`. |
| `--group-size 1` | Accepted (leader-only transaction; seals immediately; namespace/priority still honoured). |
| `--bypass-fifo` + any of `--group`, `--group-size`, `--priority`, `--global`, `--sideline`, `--no-sideline`, `--hold`, `--group-timeout`, `--hold-timeout` | Fatal, exit `2` (message in §6.3). |
| `--sideline` + `--no-sideline`/`--hold` | Fatal, exit `2`: contradictory traffic policy on a single command. |
| `--priority` $> $ `priority_max_k` | Fatal, exit `2`. |

### 7.4 Hold Semantics & Expiry — **[R4]**

`--hold` is a *head-of-line fence for Baseline traffic* (§3.5). Its lifecycle:

1. Leader locks the contract with `--hold` → group is Sealed-Waiting **and** fencing.
2. Group completes → fence lifts; the group dispatches in arrival order; waiting Baseline transactions follow in order.
3. **Hold expiry** (`hold_timeout`) with the group incomplete: governed by `hold_expiry_policy`:
   * **`release_and_degrade`** *(default)* — the fence is lifted; the group **downgrades to `--sideline`** and continues waiting until `group_timeout`, then is flushed as a degraded group. Desktop traffic is never held longer than `hold_timeout`.
   * **`abort_group`** — reproduces the earlier-draft behavior: the entire group is aborted (exit `1`, diagnostic dump), the fence lifts.
4. `hold_timeout = none` disables expiry (I6 waived; user-accepted risk). Recovery requires `SIGINT` on the **leader/any CLI of the group** or `spawn-at ctl disband <group>` / `spawn-at ctl release-hold`.

`hold_timeout` and `group_timeout` are independent; a configuration with `hold_timeout > group_timeout` simply means the group degrades first and the hold ends with it.

### 7.5 Group Lifecycle Matrix

| Group type | Trigger condition | Traffic behavior | Lifecycle / termination |
| :--- | :--- | :--- | :--- |
| **Leader-First** | First arriving frame carries `--group-size N`. | Contract locks immediately → Sealed-Waiting; moves to the **Sidelined Pool** (yields the lock) unless `--hold`. | Ready when $N$ members register; otherwise degraded flush on `group_timeout`. |
| **Worker-First (Late Leader)** | First frame has `--group` but no `--group-size`. | **Registering**: holds (fences) traffic on a short rolling timer (`group_register_timeout_ms`) awaiting a leader. | Leader arrives within the window → contract locks, group moves to Sealed-Waiting (sidelined or held). |
| **Leaderless (Ad-hoc)** | Workers check in; **no leader** arrives before the rolling timer expires. | Holds traffic for `group_register_timeout_ms` per check-in (rolling, capped like auto-groups). | On expiry: seals dynamically at $N = \text{registered\_count}$ and launches together. |
| **Strict Hold Group** | Leader specifies `--hold` / `--no-sideline`. | **Blocks all other Baseline `spawn-at` commands** across the desktop queue (Priority lane and `--bypass-fifo` are exempt). | Ready when $N$ arrive → dispatch; or hold expiry per §7.4. |
| **Auto-Group** *(implicit)* | `&` burst, §4.2. | Registering window fences later arrivals; no Sealed-Waiting phase. | Seals at window close; $N$ = members present. |
| **Solitary** *(implicit)* | Any ungrouped command. | Seals immediately (fast path). | Size 1; standard Phase 2. |
| **Priority Group** | Any of the above with `--priority`. | Priority lane; bypasses idle Phase 1 transactions. | Same lifecycle; ordering by §3.5. |

### 7.6 Exit Codes

| Code | Meaning |
| ---: | :--- |
| `0` | Success **or** degraded-but-launched (a warning is printed to `stderr`). |
| `1` | Transaction aborted by policy (`abort_group`, leader-conflict, size-violation, hold-expiry abort). Diagnostic dump printed. |
| `2` | CLI usage/validation error (no IPC occurred). |
| `3` | Coordinator unavailable **and** `coordinator_failure_policy = "fail_closed"`. |
| `4` | Extension unavailable/incompatible **and** `extension_unavailable_policy = "fail_closed"`. |
| `5` | Timed out waiting for settle (`map_timeout_ms`); application was launched. |
| `70` | Internal error (protocol violation, oversize frame). |
| `130` | Cancelled by `SIGINT`. |

### 7.7 Operator Controls (`spawn-at ctl …`)

Background (`&`) jobs cannot be interrupted with Ctrl+C from the same terminal, so the deadlock recovery path of edge case #6 needs a second door.

| Command | Effect |
| :--- | :--- |
| `spawn-at ctl status` | Dumps lanes, transactions, lock holder, holds, timers (human or `--json`). |
| `spawn-at ctl disband <group> [--global]` | Immediately cancels a group; all members receive `ABORT`/exit; the lock is dropped. |
| `spawn-at ctl release-hold` | Lifts every active fence without aborting groups (they downgrade to sideline). |
| `spawn-at ctl trace [on\|off]` | Toggles the verbose trace ring buffer (§12.1). |

---

## 8. Concurrent Leader Arbitration & Conflict Policies

When multiple commands are chained with `&` they race to the socket:

```bash
spawn-at spawn --group dev --group-size 2 gnome-terminal & \
spawn-at spawn --group dev --group-size 3 code &
```

### 8.1 The Serialization Barrier

The coordinator serializes all handshakes through a **single dispatch lock — the event loop itself (I2)**. Two processes cannot be decoded at the same instant: process A's frame is unpacked a few microseconds ahead of B's.

**Linearization point.** `arrival_seq` is assigned **when a complete `HELLO` frame is decoded**, *not* at `accept()`. (In an async design a slow client can `connect()` first yet send later; ordering by accept would let it win leadership with a frame that arrives second.) A `handshake_timeout_ms` bounds connections that never send a frame, so a stuck client cannot hold a slot, and no read ever blocks the loop (I10).

### 8.2 First-Claim Authority Invariant (I4)

Leadership is granted **strictly to the first frame, in linearization order, that carries `--group-size`**. The winner sets the **immutable contract**: target size, sideline/hold policy, timeouts, priority term. Any later frame carrying `--group-size` is a *contending secondary leader*.

### 8.3 Warned Leader Deduplication (Identical Contracts)

When a secondary frame carries a `--group-size` **equal** to the locked leader's ($Size_A == Size_B$):

* It is **never silently ignored** — identical declarations commonly mask sloppy scripts that break the moment one command's parameter is edited and the other is forgotten.
* Execution **proceeds** (sizes agree); the secondary is **stripped of leader authority**, demoted to worker, and a non-fatal warning goes to `stderr`:

```text
[spawn-at] WARNING: Redundant leader declaration on group 'dev'.
[spawn-at] PID 78103 also declared '--group-size 2' (already claimed by Leader PID 78102).
[spawn-at] Notice: Execution continued because sizes matched, but this is fragile.
           Only ONE process per group should declare --group-size.
```

Any *other* leader flags on the secondary (`--hold`, timeouts) that differ from the locked contract are ignored with the same warning block.

### 8.4 Conflict Types and Policy Selection

Two independent policies in `config.toml`, each accepting `"abort_group"`, `"demote_and_warn"`, `"eject_and_isolate"`:

1. **Leadership Usurpation** (`leader_conflict_policy`): two distinct processes declare `--group-size` (or act as leader) in the same group.
2. **Contract Size Violation** (`size_violation_policy`): the declared size is mathematically impossible given current registrations — e.g., `cmd1` and `cmd2` check in as workers, then `cmd3` arrives declaring `--group-size 2` (total would be $3 > 2$).

#### Resolution Matrix — Leader Races

| Condition | `leader_conflict_policy` | Action taken |
| :--- | :--- | :--- |
| **Identical contracts**<br>$(Size_A == Size_B)$ | *Any* | **Warned Leader Deduplication**: redundant agreement; authority stripped, demoted to standard worker, warning to `stderr`. Contract unchanged. |
| **Contradictory contracts**<br>$(Size_A \neq Size_B)$ | `abort_group` *(default)* | **Immediate transactional abort**: group poisoned; **all** members (not only the two contenders) receive `ABORT` over IPC, exit `1`, and print the command-history dump. Nothing spawns. |
| **Contradictory contracts**<br>$(Size_A \neq Size_B)$ | `demote_and_warn` | **Stripped authority**: secondary's `--group-size`, `--hold`, timeout flags discarded; demoted to worker. If room remains ($\text{count} \le Size_A$) it joins; **otherwise it falls back to `eject_and_isolate`**. Warning to `stderr`. |
| **Contradictory contracts**<br>$(Size_A \neq Size_B)$ | `eject_and_isolate` | **Group protected, offender ejected**: original group unaffected and still waiting; offender leaves the group, **bypasses the assembly barrier**, and executes immediately as a standalone Baseline transaction. |

#### Resolution Matrix — Size Violations

| Condition | `size_violation_policy` | Action taken |
| :--- | :--- | :--- |
| Declared $N <$ members already present | `abort_group` *(default)* | Whole group poisoned, exit `1`, size-violation dump (§8.5). |
| Declared $N <$ members already present | `demote_and_warn` | Leader's `--group-size` is **ignored** (demoted to worker); group seals under the leaderless rules (`N = registered_count`) after its window. |
| Declared $N <$ members already present | `eject_and_isolate` | The **offending leader** is ejected and runs standalone; the pre-existing workers continue as a leaderless group. |
| Late member would exceed locked $N$ | *(same three)* | Abort / demote-to-ejection / eject the excess member respectively. |

### 8.5 Policy Behaviors & Diagnostic Telemetry (exact output)

#### Policy 1 — `abort_group` (strict transactional safety, default)

The entire group transaction is poisoned and deleted. All pending CLIs unblock and exit `1`. Nothing spawns. The CLI emits a full history buffer identifying the **winning leader**, the **rogue PID**, and complete command lines:

```text
[spawn-at] FATAL: Leadership contention detected on group 'dev'.
[spawn-at] Transaction aborted to prevent visual layout corruption.

Conflict details:
  - First Leader (Registered first): PID 78102 declared --group-size 2
  - Usurping Leader (Rejected):      PID 78103 declared --group-size 3

Registered Group History Dump:
  [#1] PID 78102 (LEADER): spawn-at spawn --group dev --group-size 2 gnome-terminal
  [#2] PID 78103 (ROGUE) : spawn-at spawn --group dev --group-size 3 code

Action: Check your script or keybinding. Only ONE process in a group should carry --group-size.
```

**Abort tombstone.** After an abort the group key is **tombstoned** for `abort_tombstone_ms` (default 500). Stragglers from the same script that connect *after* the abort — which would otherwise form a brand-new group and wait out `group_timeout` alone — receive an immediate `ABORT` carrying the original dump. (Edge case #28.)

#### Size-violation dump (`abort_group`)

```text
[spawn-at] FATAL: Contract size violation on group 'dev'.
[spawn-at] Transaction aborted to prevent visual layout corruption.

Violation details:
  - Declared by Leader PID 78105:     --group-size 2
  - Members already registered:       2 workers (+ the leader = 3)  →  3 > 2

Registered Group History Dump:
  [#1] PID 78103 (WORKER): spawn-at spawn --group dev alacritty
  [#2] PID 78104 (WORKER): spawn-at spawn --group dev code
  [#3] PID 78105 (LEADER, REJECTED): spawn-at spawn --group dev --group-size 2 firefox

Action: Declare --group-size equal to the real member count, or remove it to let the group seal automatically.
```

#### Policy 2 — `demote_and_warn` (permissive reintegration)

```text
Warning: PID 64910 attempted to usurp leadership on group 'dev'.
Leader flags stripped. Process demoted to worker member.
```

If demotion would still produce `current_count > locked_target_size`, the process cannot join and the system **falls back to `eject_and_isolate`**.

#### Policy 3 — `eject_and_isolate` (non-destructive removal)

```text
Warning: PID 64910 rejected from group 'dev' due to size/leadership conflict.
Executing as an isolated single spawn.
```

### 8.6 Diagnostic Catalog (all `stderr` strings)

| Event | Severity | Text (abridged) | Exit |
| :--- | :--- | :--- | :---: |
| Redundant leader | warn | `WARNING: Redundant leader declaration on group '<g>'.` | 0 |
| Leader contention (abort) | fatal | `FATAL: Leadership contention detected on group '<g>'.` + dump | 1 |
| Size violation (abort) | fatal | `FATAL: Contract size violation on group '<g>'.` + dump | 1 |
| Demote | warn | `Warning: PID <p> attempted to usurp leadership…` | 0 |
| Eject | warn | `Warning: PID <p> rejected from group '<g>'…` | 0 |
| Infinite hold | warn | `Warning: Infinite hold timeout requested…` | — |
| Flag authority | fatal | `Error: '--hold' is a leader flag and requires '--group-size'.` | 2 |
| Bypass incompat. | fatal | `Error: Incompatible flags. '--bypass-fifo' cannot be used with …` | 2 |
| Degraded group flush | warn | `Warning: group '<g>' timed out with <k>/<N> members; launching degraded.` | 0 |
| Hold expiry downgrade | warn | `Warning: hold on group '<g>' expired after <ms> ms; continuing as sidelined.` | 0 |
| Extension unavailable | warn | `Warning: spawn-at shell extension not reachable; launching without layout control.` | 0 / 4 |
| Coordinator unavailable | warn | `Warning: coordinator unreachable; launching un-coordinated.` | 0 / 3 |
| Spawn failed (member) | warn | `Warning: member PID <p> failed to exec '<cmd>' (<errno>); group continues with <k> windows.` | 0 |

---

## 9. Compositor-Side Specification (GNOME Shell Extension)

The coordinator decides **when**; the extension decides **where, how, and who has focus**. This section specifies the extension half so the protocol of §3 is implementable end to end.

### 9.1 Claims & the Identity-Matching Ladder

When `ArmSpawn(batch)` is acked (2A), the extension holds one **claim** per batch entry. When Mutter creates a window (`global.display` `window-created`), the extension tries to match it to a claim using the first rule that yields a unique answer:

| Rung | Match basis | Reliability | Notes |
| :---: | :--- | :--- | :--- |
| 1 | **Activation token** (`xdg-activation-v1` / startup-notification id) carried by the spawned process | Highest | Delivered via env at spawn; **stripped by D-Bus-activated servers** (see edge #9). |
| 2 | **PID / ancestor-PID** (`Meta.Window.get_pid()` equals, or descends from, a PID reported in `SPAWNED`; ancestry via `/proc/<pid>/stat` walk) | High | Works for direct children and `exec`-style wrappers. Fails for single-instance and server-model apps. |
| 3 | **Canonical WM class** (`driver.resolve_id`) in **class-FIFO order** | Medium | Reliant on map order; guarded by the interlock below. |

**Type filter.** Claims match only `Meta.WindowType.NORMAL` windows (splash screens, dialogs, and tooltips of the same class are ignored), so an application's splash window cannot consume the claim intended for its main window (edge #32).

**PID-opaque classes and the same-class interlock.** If two outstanding claims could resolve to the same class **and** that class is *PID-opaque* (server-model/single-instance apps — default list `["org.gnome.Terminal", "org.gnome.Nautilus"]`, extended at runtime when a PID match fails once for a class), the coordinator **serializes their 2B (spawn) stages**: the second transaction's spawn waits until the first's claim for that class is fulfilled or expired. For PID-resolvable classes (e.g., `alacritty`, which *is* its own window owner) no interlock is needed, so two simultaneous `alacritty` launches from different shells do not wait on each other.

**Claim lifecycle & TTL (I9).** Every claim carries `claim_ttl_ms` (default 5000, always $\ge$ `map_timeout_ms`). On expiry the claim is deleted, `ClaimExpired(batch_id, class)` is emitted, and the batch settles **degraded** with the members that did map. This prevents the *stale-claim hijack*: a single-instance app that merely focuses an existing window (so no new window ever maps) must not leave a claim that later teleports an unrelated, manually opened window of the same class (edge #33).

**Express Claim Table.** `BypassArm` entries live in a separate table. Matching order: PID/token first; class matching is allowed only when **no queued-batch claim for that class is outstanding**, otherwise the express claim waits for PID/token resolution.

### 9.2 D-Bus Interfaces

Two well-known names; `Hello` negotiates the protocol before any traffic.

**`org.spawnat.Shell1`** (owned by the extension, object `/org/spawnat/Shell1`)

| Member | Direction | Semantics |
| :--- | :--- | :--- |
| `Hello(client_proto, epoch) → (ext_proto, caps)` | C → Ext | Version/capability negotiation. A major-version mismatch causes the coordinator to run in degraded mode (§10.3) and print *"extension protocol mismatch — log out and back in to load the updated extension"* (GNOME on Wayland cannot reload extensions without restarting the session). |
| `ArmSpawn(epoch, batch_id, entries[], options) → (ok, reason)` | C → Ext | **One call per batch.** Entries: claim identity, anchor, size hints, margins, monitor target, focus role. Stale `epoch` ⇒ rejected (§10.4). |
| `Abort(epoch, batch_id, reason)` | C → Ext | Drop claims; uncloak any already-mapped member **in place** (no geometry applied) so no window is stranded. |
| `BypassArm(entry) → (ok \| Throttled)` | CLI → Ext | Out-of-band express claim (§6). |
| `Ping() → (epoch, load)` | C → Ext | Liveness for hang detection. |
| Signal `BatchSettled(batch_id, status, detail)` | Ext → C | `status ∈ {ok, degraded, timeout}`; `detail` lists unfulfilled claims. |
| Signal `ClaimExpired(batch_id, class)` | Ext → C | Drives degraded handling. |

**`org.spawnat.Coordinator1`** (owned by the coordinator)

| Member | Direction | Semantics |
| :--- | :--- | :--- |
| `RequestSettle(batch_id) → (granted)` | Ext → C | Async-reply: the reply **is** the lock grant (2D). Reply is immediate when the lock is free. |
| `SettleDone(batch_id)` | Ext → C | Redundant with `BatchSettled` when the signal is lost; idempotent. |

All D-Bus calls from the coordinator use `dbus_call_timeout_ms`; there is **at most one outstanding `ArmSpawn`** (the lock is exclusive), so coordinator-originated bus load is $O(1)$ regardless of burst size.

### 9.3 Cloak, Settle & Uncloak

**Cloak (2C).** On claim match, the extension immediately sets the `Meta.WindowActor` to `opacity = 0` **and** makes it input-inert: it is non-reactive, receives no programmatic focus, and is excluded from the Alt-Tab/overview lists while cloaked. (Opacity alone does not stop Mutter from delivering clicks to the surface beneath the pointer; without input-inertness an invisible window could swallow the user's first click — edge #29.) Cloaked windows are tagged with a **cloak deadline**, $t_{\text{map}} + \texttt{cloak\_max\_ms}$ (I8).

**Settle (2D) algorithm**, run when `RequestSettle` is granted:

```text
 1. RE-RESOLVE   monitor, work area, scale (late binding — §9.4; arm-time values may be stale)
 2. CONFIGURE    for each member: request target size via move_resize_frame(); wait for the
                 client's ack/commit that reports the new size (size-changed + first-frame),
                 bounded by configure_ack_timeout_ms   ← "uncloak-on-ack"
 3. CLAMP        read effective frame size; apply min-size / max-size clamps
 4. ANCHOR       compute edges (§9.4); re-evaluate margins POST-clamp (edge #10); move_frame()
 5. FRAME-SETTLE wait settle_frames (default 1) compositor frame(s) so late sub-surface commits
                 (video/GL subsurfaces, GTK4/Chromium overlays) land before reveal
 6. UNCLOAK      ease opacity 0 → 255 over uncloak_ramp_ms (~15–30 ms ≈ 1–2 frames @ 60 Hz),
                 all members in the same frame callback → "same-frame appearance"
 7. FOCUS        hand focus to the designated member via Focus Authority (§9.5), unless a
                 soft-yield barrier is active
 8. SIGNAL       emit BatchSettled(batch_id, status)
```

**Uncloak-on-ack** is the actual cure for relayout stutter: a Wayland client first maps at its *own* default size, then receives a configure from the compositor and re-commits. Revealing before that re-commit shows the window at the wrong size for one or more frames. Waiting for the matching size-change eliminates it.

**Group barrier.** Steps 1–8 run only when **all** claims for the batch are fulfilled, or when the batch is degraded (timeout, spawn failure, claim expiry). Members that mapped early stay cloaked in the meantime. This barrier is what guarantees atomic group appearance irrespective of spawn skew.

### 9.4 Placement Math

**Coordinate space.** All rectangles are held in **global stage logical coordinates** (as returned by `get_work_area_for_monitor(i)`), never in monitor-local coordinates, so mixed-DPI and multi-monitor layouts do not require per-monitor translation. Placement uses the **frame rect** (`get_frame_rect()`), not the buffer rect (which includes invisible client-side-decoration shadows).

**Anchoring.** Given work area $W = (x_0, y_0, w, h)$, margins $(m_t, m_r, m_b, m_l)$, requested size $(w_t, h_t)$ and minimum size $(w_{\min}, h_{\min})$:

$$w' = \min\!\big(\max(w_t, w_{\min}),\; w - m_l - m_r\big), \qquad h' = \min\!\big(\max(h_t, h_{\min}),\; h - m_t - m_b\big)$$

| Anchor (horizontal) | $x$ | Anchor (vertical) | $y$ |
| :--- | :--- | :--- | :--- |
| left | $x_0 + m_l$ | top | $y_0 + m_t$ |
| right | $x_0 + w - m_r - w'$ | bottom | $y_0 + h - m_b - h'$ |
| center | $x_0 + \lfloor (w - w')/2 \rfloor$ | center | $y_0 + \lfloor (h - h')/2 \rfloor$ |

**Post-clamp margin re-evaluation (edge #10).** Toolkit minimum sizes can *expand* a window after the first computation (e.g., a terminal that refuses to shrink below its minimum rows), pushing a bottom/right-anchored edge past its margin. `_applyAnchoredPosition` therefore evaluates **`window.get_min_size()` after the clamp** (per the current implementation) and **recomputes $x$/$y$ from the final $w'$, $h'$**. Conflict rule if the minimum size exceeds the available area: the **anchored edge's margin wins**; the opposite edge may overflow the work area. The anchored edge is never pushed.

**Edge-based construction (fractional scaling safe).** With fractional scaling (e.g., 125 %, 150 %) physical edges are $\text{round}(\text{logical} \times s)$. Computing position and width independently and summing them produces 1-px gaps or overlaps between adjacent tiles. The extension therefore constructs rectangles from **integer edges** and derives size by subtraction:

$$x_{\text{mid}} = x_0 + \Big\lfloor \tfrac{w}{2} \Big\rfloor, \qquad \text{left tile: } [\,x_0 + m_l,\; x_{\text{mid}}\,), \qquad \text{right tile: } [\,x_{\text{mid}},\; x_0 + w - m_r\,)$$

The shared edge $x_{\text{mid}}$ is a **single value** used by both tiles (**Single-Source Edges**), so physical rounding is identical on both sides and no seam appears at any scale factor. When `fractional_scale_snap = true`, edges are additionally snapped to the nearest logical integer whose physical image $e \times s$ is itself integral where one exists within ±1 px.

**Late binding.** The target monitor, work area (panels/docks change struts), and scale are resolved at **arm time** (for claim bookkeeping) and **re-resolved at Settle step 1** (edge #18). If the target monitor has disappeared, the fallback is the primary monitor.

### 9.5 Focus Authority (Single Writer)

**For every window that `spawn-at` has claimed, the extension is the only writer of focus.** The application's own focus requests are not suppressed (that would break applications), but `spawn-at` itself never issues competing activations. All focus-class operations pass through one function:

```text
function requestFocus(window, origin):
    if origin == GROUP and now_monotonic < bypass_active_until:
        deferred_focus = (batch_id, window); return SUPPRESSED         # §6.5
    focus_epoch += 1
    window.activate(global.get_current_time())
```

`focus_epoch` is bumped on every grant so that a late, queued request from an *older* epoch can be recognized and discarded rather than stealing focus after a newer decision. The **designated focus member** of a group is `group_focus_target` (default: the leader, else the first member to register).

### 9.6 Extension Lifecycle Obligations

* `enable()`: claim the bus name, register interfaces, install signal handlers, publish `epoch=0` until `Hello`.
* `disable()` (also on screen-lock extension-disable modes): **uncloak every cloaked window, drop all claims, clear the bypass flag, disconnect all signals and timers.** (Required by the extension review guidelines and by I8.)
* **Dead-man watch.** The extension watches `NameOwnerChanged` for `org.spawnat.Coordinator1`; loss of the owner triggers *Fail-Safe Completion* (§10.1).
* **Feature detection.** Mutter/Shell APIs shift between major versions; the extension probes the signals/methods it needs at `enable()` and advertises them in `caps`. Missing capability ⇒ it downgrades the feature (e.g., no input-inert cloak ⇒ `cloak_mode = "opacity"` only) and reports it.

---

## 10. Failure Domains & Recovery

**Guiding principle (I7):** every failure resolves in the user's favor — the application launches and no window stays invisible. `spawn-at` is a *layout optimizer*, never a gatekeeper.

### 10.1 Failure-Domain Matrix

| # | Component | Failure | Detection | Immediate effect | Recovery |
| :---: | :--- | :--- | :--- | :--- | :--- |
| F1 | Coordinator | **Crash while idle** | Next `connect()` fails / socket stale | None yet | **Socket activation** (`spawn-at-coordinator.socket` systemd user unit) restarts it on the next connection; stale socket file is detected by a connect-probe and unlinked before `bind()`. |
| F2 | Coordinator | **Crash during Phase 1** | CLIs see EOF on the socket | Parked CLIs unblocked | Each CLI follows `coordinator_failure_policy`: **`fail_open`** (default) → spawns un-coordinated after a one-line warning; `fail_closed` → exit `3`. |
| F3 | Coordinator | **Crash mid-Phase 2 (2A/2B)** | `NameOwnerChanged` at the extension; EOF at CLIs | Claims installed but spawns possibly unissued | Extension **Fail-Safe Completion**: complete any in-flight settle, then **uncloak all cloaked windows in place**, delete claims; CLIs spawn per policy. |
| F4 | Coordinator | **Crash mid-2C (MapAwait)** | same | Windows cloaked, no one will request settle | Extension runs **autonomous settle**: it has the full batch, so it settles mapped members itself after `degrade_grace_ms` and expires the rest. Cloak deadline (I8) is the backstop. |
| F5 | Coordinator | **Crash mid-2D (Settling)** | same | Settle in progress | Extension **finishes** the settle (never abandons half-applied geometry), then enters fail-safe. |
| F6 | Coordinator | **Hang** (event loop stalled) | `Ping()` misses ×`heartbeat_miss_limit` | Queue frozen | Extension treats the coordinator as dead (as F3–F5) and *raises* a notification; CLIs time out at `coordinator_connect_timeout_ms`/barrier watchdog and follow F2. |
| F7 | Extension | **Absent / disabled at start** | `ServiceUnknown` on first D-Bus call | No layout control | `extension_unavailable_policy`: `fail_open` → raw spawn + warning (file-lock fallback below); `fail_closed` → exit `4`. |
| F8 | Extension | **Disabled/crashes mid-batch** | Bus-name loss; `ArmSpawn` timeout | Cloaked windows possibly stranded | `disable()` uncloaks everything (§9.6). On Wayland a Shell crash ends the session, so no state outlives it. |
| F9 | Protocol | **Version skew** (extension updated on disk but old code still loaded) | `Hello` mismatch | Incompatible batches | Degraded mode (F7 semantics) + explicit "log out and back in" message; **no partial protocol speaking**. |
| F10 | D-Bus | **Broker disconnect/restart** | Connection error | Arm/Settle calls fail | Coordinator reconnects with exponential back-off; in-flight batches are failed as F3–F5; CLIs follow policy. |
| F11 | App | **Exec failure** (typo, ENOENT) | `SPAWNED{errno}` | A claim that will never be fulfilled | Coordinator `Abort`s that single claim; batch **degrades immediately** to the members that did launch (no waiting for `map_timeout_ms`). |
| F12 | App | **Never maps a window** (daemon, headless, single-instance reuse) | `map_timeout_ms` / `claim_ttl_ms` | Group would wait forever | Batch degrades; CLI exits `5` with the app still running; claim deleted (I9). |
| F13 | System | **Suspend / resume** | logind `PrepareForSleep` | `CLOCK_MONOTONIC` does not advance during suspend; timers resume late | `on_resume_policy` (default `flush_degraded`): assembling groups are flushed degraded, holds released. |

**Fallback mutual exclusion (F2/F7).** Without a coordinator the CLIs may serialize their own raw D-Bus arming with `flock(2)` on `/run/user/$UID/spawn-at-arm.lock` (the "legacy arm lock"). This restores *mutual exclusion* of arm sections but **not FIFO fairness** (`flock` is unordered) and not grouping; it is a degraded mode, never the primary design.

### 10.2 CLI-Level Failures

| Event | Detection | Behavior |
| :--- | :--- | :--- |
| **`SIGINT`/`SIGTERM` while parked** | CLI handler → `CANCEL` | If the sender is the **leader** or belongs to a `--hold` group → **full group disband** and lock drop (members exit `1`, "disbanded"). Otherwise only that member is withdrawn. |
| **`SIGKILL` / crash while parked** | EOF on its socket | Member removed; the contract is **not** auto-reduced; the remaining members proceed to `group_timeout` and a degraded flush. (A dead leader's contract still stands — the group is not punished for it.) |
| **Peer exits during Phase 2** | EOF + `pidfd` | Its claim is expired immediately; the batch degrades. |
| **Slow/stuck handshake** | `handshake_timeout_ms` | Connection closed, logged; no queue slot consumed. |
| **Oversize/malformed frame** | Decoder | Connection closed, `70`, trace entry; no state mutated. |

### 10.3 Degradation Ladder

```text
 L0  Full:   coordinator + extension healthy                    → deterministic layout
 L1  Lock:   coordinator down, extension up                     → CLIs serialize arming via arm.lock; no grouping
 L2  Raw:    extension unavailable/mismatched                   → raw spawn, no placement (warning)
 L3  Closed: fail_closed selected                               → exit 3/4, nothing launches (opt-in only)
```

### 10.4 Coordinator Epochs & Restart Safety

Each coordinator process draws a random 64-bit **epoch** at start. Every `ArmSpawn`, `Abort`, and `RequestSettle` carries `(epoch, batch_id)`. After a restart the extension rejects stale batches from a previous epoch (and runs fail-safe for any orphans), so a reborn coordinator can never double-apply or collide with its predecessor's `batch_id` space.

### 10.5 Resource Limits & Backpressure (D-Bus / UDS Saturation)

A script that fires 200 `&` jobs must degrade gracefully, not stall the session.

| Resource | Limit (config) | Overflow behavior |
| :--- | :--- | :--- |
| UDS accept backlog | `listen_backlog` (128) | Excess connects get `ECONNREFUSED`/`EAGAIN` → CLI backs off 5 ms × 3 then follows F2 (`fail_open`). |
| Queue depth | `max_queue_depth` (256) | New frames receive `ABORT(queue_full)`; CLI follows `fail_open` (launches raw). |
| Group / burst size | `max_group_size` (32) / `auto_group_max_members` (16) | The burst seals at the cap; later members form a *new* transaction queued behind. |
| In-flight D-Bus calls | **1** `ArmSpawn` at a time (lock exclusivity) | Not reachable. Bus quotas are therefore not stressed by the coordinator; the only multi-writer, `BypassArm`, is token-bucketed (§6.6). |
| D-Bus payload | One batch = one call; entries are compact | No per-window/per-property chatter. |
| Frame size | 64 KiB | Connection closed. |

---

## 11. Comprehensive Edge-Case Register

**Part I** preserves the canonical ten-point register in full. **Part II** records every additional failure mode, race condition, and bottleneck found during the system audit; each entry points to the section where its mitigation is specified in-line.

### 11.1 Part I — Canonical 10-Point Register

| # | Scenario | Manifestation | Mitigation Strategy |
| :---: | :--- | :--- | :--- |
| **1** | **The Late Leader** | Worker runs at $t = 0\text{ ms}$; Leader runs at $t = 15\text{ ms}$. | The worker holds the registration window on the `group_register_timeout_ms` rolling timer (40 ms). The leader's arrival **upgrades the contract seamlessly** without breaking FIFO registration order; the group then moves to Sealed-Waiting. |
| **2** | **Size Exceeded Before Leader Check-In** | `cmd3` arrives with `--group-size 2` after 2 workers are already waiting. | A mathematical paradox ($3 > 2$). Handled strictly by **`size_violation_policy`** (Abort / Demote / Eject), §8.4–8.5. |
| **3** | **Multiple Contradictory Leaders via `&`** | `cmd1` declares `--group-size 2`, `cmd2` declares `--group-size 4`. | Handled strictly by **`leader_conflict_policy`** via atomic IPC serialization; first-claim wins (I4); full diagnostic dump with winning vs. rogue PIDs, §8. |
| **4** | **Identical Leader Redundancy (Warned Deduplication)** | Both `cmd1` and `cmd2` pass `--group-size 2`. | **Warned Leader Deduplication**: execution continues (sizes match), the secondary is demoted to worker, and an explicit `stderr` warning flags the fragile scripting, §8.3. |
| **5** | **Missing Group Member (Crash / Typo)** | Script declares `--group-size 3`, but only 2 commands execute. | **`group_timeout_ms`** triggers a **degraded release**: existing members flush through the IPC gate and launch as a degraded group, preventing a permanent queue hang. Warning printed; exit `0`. |
| **6** | **Deadlock via `--hold-timeout none`** | Leader sets infinite hold, a member never arrives; the system queue freezes. | The user explicitly opted into the risk (warning printed, §7.2). Recovery: manual **`SIGINT`** (Ctrl+C) on the hanging CLI, which notifies the IPC socket to drop the lock and **disband the group**; for backgrounded jobs use `spawn-at ctl disband <group>` / `release-hold`, §7.7. |
| **7** | **Cross-Terminal Namespace Collision** | Two terminal tabs run scripts using `--group dev`. | **PTY / Session auto-scoping**: group keys are internally hashed (`dev:pts-2` vs `dev:pts-4`; precisely $\text{hash}(\text{ctty\_dev},\text{SID},\text{SID.start\_time})$). Tabs cannot corrupt each other unless `--global` is passed. |
| **8** | **Concurrent D-Bus `ArmSpawn` Inversion** | Two processes fire concurrently (`&`) and race to GNOME Shell's D-Bus interface. | The coordinator dispatches settled instructions to D-Bus **sequentially through a single thread, one `ArmSpawn` at a time, before** releasing the spawn gate — strict FIFO ordering, §3.3 / §9.2. |
| **9** | **D-Bus Daemon Token Stripping** | `gnome-terminal-server` drops custom activation tokens, returning a null `startup_id`. | **Fallback to the canonical WM Class**: revert the D-Bus `target_id` to the driver's canonical class (`driver.resolve_id`), relying on the extension's class-FIFO queue and Batch Mutex instead of fragile environment tokens (matching ladder rung 3, §9.1; same-class interlock for PID-opaque classes). |
| **10** | **Margin Violations Under Dynamic Clamping** | Toolkit sizing clamps expand the window height, pushing the bottom edge out of bounds. | **Post-clamp evaluation**: `_applyAnchoredPosition` evaluates `window.get_min_size()` after the clamp and **recalculates the $Y$ position dynamically**; anchored-edge margin wins on irreducible conflict, §9.4. |

### 11.2 Part II — Audit-Derived Additions

| # | Scenario | Manifestation | Mitigation Strategy | See |
| :---: | :--- | :--- | :--- | :---: |
| **11** | **Coordinator crash mid-Phase 2** | Claims installed, windows cloaked, nobody to request settle or release the lock. | Extension **Fail-Safe Completion**: finish any in-flight settle, autonomously settle mapped members, uncloak everything in place; CLIs follow `coordinator_failure_policy`. Cloak deadline is the backstop (I8). | §10.1 F3–F5 |
| **12** | **Extension absent, disabled, or version-skewed** | `ServiceUnknown`/mismatch on first D-Bus call; Wayland cannot hot-reload extensions. | `Hello` negotiation; degraded L2 (raw spawn + explicit "log out and back in" message) or `fail_closed`. No partial protocol speaking. | §9.2, §10.3 |
| **13** | **Slow-mapping or never-mapping application** | Electron takes seconds to map; a daemon never maps. A lock spanning map-wait would freeze the queue. | **Lock Split** (R3): map-wait is outside the lock; `map_timeout_ms` degrades the batch; CLI exits `5` with the app still running. | §3.3 |
| **14** | **Same-class / PID-opaque ambiguity** | Two `alacritty` (or two `gnome-terminal-server` windows) map out of order; the class-FIFO swaps their geometry. | Matching ladder (token → PID → class); **same-class interlock** serializes spawn for PID-opaque classes only. | §9.1 |
| **15** | **Job-control PGID divergence & compositor-parent false merge** | `&` jobs in an interactive shell have different PGIDs (gate never merges); two keybinding presses share `PPID = gnome-shell` (false merge). | Burst key $(\text{uid},\text{SID},\text{PPID})$ with compositor/systemd/PID 1 excluded; PGID diagnostic only (R1). | §4.2 |
| **16** | **Debounce latency tax & rolling-window starvation** | Fixed 40 ms on every solitary command; an endless `&` loop extends the window forever. | **Adaptive fast path** (R8) for siblings-free commands; hard cap `auto_group_max_window_ms` and `auto_group_max_members`. | §4.2 |
| **17** | **Fractional-scaling seams & mixed-DPI coordinates** | 1-px gap/overlap between tiles at 125 %/150 %; wrong coordinates when mixing monitor-local and global spaces. | **Edge-based construction**, Single-Source Edges, global logical coordinates only, frame rect (not buffer rect). | §9.4 |
| **18** | **Monitor hotplug / workspace / strut change between arm and settle** | Target monitor unplugged; dock changes work area; user switches workspace. | **Late binding**: re-resolve monitor, work area, scale at Settle step 1; fallback to primary monitor. | §9.4 |
| **19** | **Configure-ack race & late sub-surface content** | Window revealed at client default size for 1–2 frames; GL/video subsurfaces commit after the toplevel. | **Uncloak-on-ack** + `settle_frames` frame(s) before reveal; bounded by `configure_ack_timeout_ms`. | §9.3 |
| **20** | **Bypass flood / held hotkey** | Hundreds of `BypassArm` calls saturate the extension. | Per-class token bucket (`bypass_rate_limit_per_sec`); `Throttled` ⇒ raw spawn without placement. | §6.6 |
| **21** | **Bypass vs. queued-batch class collision** | A bypass `alacritty` steals a group's `alacritty` claim (or vice versa). | Separate **Express Claim Table**; PID/token first; class matching only when no queued claim for that class is outstanding. | §9.1 |
| **22** | **Focus orphan after early-dismissed bypass window** | `rofi` closes within 40 ms; the group's focus call was suppressed ⇒ no window focused. | **Deferred Focus Restoration**: on barrier expiry restore `deferred_focus` if the bypass window no longer holds focus. | §6.5 |
| **23** | **Priority lane starves Baseline lane** | A script spamming `--priority 0` freezes normal launches. | **Starvation guard**: after `max_consecutive_priority` dispatches serve one Ready Baseline entry. | §5.6 |
| **24** | **UDS / D-Bus saturation by mass `&`** | 200 background jobs overflow the accept backlog or queue. | Bounded backlog/queue/group size; `fail_open` on overflow; **single outstanding** `ArmSpawn`; token-bucketed express path. | §10.5 |
| **25** | **Stale socket / coordinator restart / epoch collision** | A restarted daemon reuses `batch_id`s the extension already saw. | Connect-probe + unlink; socket activation; random **epoch** on every call; stale epochs rejected. | §10.1, §10.4 |
| **26** | **Suspend/resume with armed timers** | `CLOCK_MONOTONIC` excludes suspend; holds and groups survive a lid-close for far too long (or time out immediately, depending on clock). | logind `PrepareForSleep` hook; `on_resume_policy` flushes assembling groups degraded and releases holds. | §10.1 F13 |
| **27** | **PTY number recycling** | `pts-2` closes and a new session reuses it; a stale group key matches the new session. | Scope key includes SID **start time**; groups are short-lived and tombstoned on completion. | §11.1 #7, §2.3 |
| **28** | **Post-abort stragglers** | After `abort_group`, late members of the same script form a *new* group and wait out `group_timeout` alone. | **Abort tombstone** (`abort_tombstone_ms`): stragglers receive an immediate `ABORT` with the original dump. | §8.5 |
| **29** | **Cloaked-window input leak (ghost clicks)** | `opacity = 0` does not stop Mutter from routing clicks to the surface under the pointer. | Cloaked windows are **input-inert** (non-reactive, no programmatic focus, hidden from Alt-Tab). | §9.3 |
| **30** | **`SIGKILL`/crash of a parked member** | Contract can no longer be met. | EOF ⇒ member removed; contract not auto-reduced; degraded flush at `group_timeout`; leader `SIGINT` ⇒ full disband. | §10.2 |
| **31** | **Member exec failure (typo'd binary)** | `ENOENT`; its claim will never be fulfilled and the group would wait `map_timeout_ms`. | `SPAWNED{errno}` ⇒ claim withdrawn; batch **degrades immediately**. | §10.1 F11 |
| **32** | **Splash screens / multi-window applications** | A splash window consumes the claim meant for the main window. | Claims match `Meta.WindowType.NORMAL` only. | §9.1 |
| **33** | **Stale-claim hijack (single-instance reuse)** | `firefox`/`code` focus an existing window; no new window maps; the lingering claim later teleports an unrelated window of that class. | **Claim TTL** (`claim_ttl_ms`, I9); `ClaimExpired` signal; degraded settle. | §9.1 |
| **34** | **Async accept/decode inversion** | A slow client `connect()`s first but sends its `HELLO` second, winning leadership by accept order. | Linearization point = **complete-frame decode**, not `accept()`; `handshake_timeout_ms`. | §8.1 |

---

## 12. Observability & Verification

### 12.1 Observability

* **Structured logging** (`tracing` → journald): every transaction carries `txn_id`, `batch_id`, `epoch`, `scope`, `lane`, `p_assigned`, `state`; every state transition is an event with a monotonic timestamp.
* **Trace ring buffer**: the last `trace_ring_entries` (default 4096) events are kept in memory, dumped on `spawn-at ctl trace` or automatically on any abort/fail-safe event.
* **`spawn-at ctl status`**:

```text
epoch 0x5f3a…  lock: HELD by batch#4412 (Settling, 11 ms)   bypass_active_until: —
Priority lane:  [0] copyq (txn 2201, Ready)   [1] rofi (txn 2202, Registering)
Baseline lane:  dev   (txn 2190, SealedWaiting 2/3, HOLD, hold_remaining 640 ms, group_remaining 1210 ms)
                firefox (txn 2199, fenced behind txn 2190)
Armed batches:  #4410 (MapAwait, 1/2 mapped, claim_ttl 3.8 s)
```

* **Extension debug HUD** (opt-in): overlays cloaked-window count, claim table size, and last settle duration, for diagnosing Phase 2 timings.

### 12.2 Verification Strategy

| Layer | Technique | Targets |
| :--- | :--- | :--- |
| **Deterministic simulator** | The coordinator state machine is a pure function of `(state, event, now)`; run with an injected virtual clock and scripted event interleavings. | I1–I6, I9; all matrices in §8; compaction; ready-set comparator. |
| **Property tests** | Random sequences of `HELLO`/timer/cancel events; assert order preservation (I5), single leader (I4), no spawn-before-contract (I1), bounded waiting (I6). | §3.5, §5.3, §8 |
| **Protocol fuzzing** | `cargo-fuzz` on the frame decoder; oversize/truncated/garbage frames. | I10, §10.2 |
| **Fault injection** | Kill coordinator at every `TxnState`; drop D-Bus replies; delay `RequestSettle`; reload extension mid-batch. | I7, I8, §10.1 |
| **Compositor integration** | Headless/nested GNOME Shell session; scripted launches of terminals, Electron apps, a GTK4 video app; frame-capture assertions that no intermediate geometry is ever presented. | R3, §9.3, §9.4 |
| **Scale/DPI matrix** | 100 %, 125 %, 150 %, 200 %, mixed-DPI dual monitors; pixel-seam assertion between adjacent tiles. | Edge #17 |
| **Soak** | 72 h of randomized launches including `--priority`/`--bypass-fifo` storms; assert $P_{\max} \le N_{\max} + K_{\max}$ and zero leaked claims/cloaks. | §5.4, I8, I9 |
| **Shell-semantics matrix** | bash/zsh/fish/dash; interactive vs script; `;`, `&`, `&&`, subshells `( … & )`, keybinding launch, `systemd-run --user`. | R1, §4.2 |

---

## 13. Configuration Reference (`config.toml`)

Location: `~/.config/spawn-at/config.toml` (mirrored in the extension Preferences UI). Every key below is optional; the values shown are the defaults. Durations are milliseconds unless noted. Unknown keys produce a warning, not an error.

```toml
# ═════════════════════════════════════════════════════════════════════════════════
# [coordination]  — Phase 1: assembly, grouping, contracts, conflicts
# ═════════════════════════════════════════════════════════════════════════════════
[coordination]

# ── Auto-Group detection for concurrent shell jobs ('&') ─────────────────────────
auto_group_enabled = true            # master switch. false => every ungrouped command is Solitary.
auto_group_window_ms = 40            # rolling debounce for implicit '&' bursts.
auto_group_max_window_ms = 250       # HARD cap measured from the first arrival of a burst (anti-starvation).
auto_group_max_members = 16          # burst seals at this many members; extras form a new transaction.
auto_group_fast_path = true          # seal immediately when /proc shows no live sibling under the same PPID.
auto_group_excluded_parents = [      # PPIDs that must never act as a burst parent (keybinding guard).
  "gnome-shell", "systemd", "gnome-session-binary",
]

# ── Explicit worker-first / leaderless groups ─────────────────────────────────────
group_register_timeout_ms = 40       # rolling window awaiting a leader (alias: legacy key 'group_register_timeout').
group_register_max_ms = 250          # hard cap for the explicit-group register window.

# ── Conflict resolution: "abort_group" | "demote_and_warn" | "eject_and_isolate" ──
leader_conflict_policy = "abort_group"   # two processes declare leadership in one group.
size_violation_policy  = "abort_group"   # declared size is impossible given current registrations.
abort_tombstone_ms = 500             # how long an aborted group key rejects late stragglers (edge #28).

# ── Group / hold timing ──────────────────────────────────────────────────────────
group_timeout_ms = 1500              # default max wait for missing members before a degraded flush.
hold_timeout_ms = 1000               # default max duration a --hold fence may block global desktop traffic.
hold_expiry_policy = "release_and_degrade"   # "release_and_degrade" | "abort_group"   (R4)

# ── Namespace scoping ─────────────────────────────────────────────────────────────
default_session_isolation = true     # --group dev in terminal A is separate from terminal B; --global shares.

# ── Admission limits ──────────────────────────────────────────────────────────────
max_group_size = 32
max_queue_depth = 256
handshake_timeout_ms = 250           # connection that never delivers a complete HELLO is dropped.
listen_backlog = 128

# ═════════════════════════════════════════════════════════════════════════════════
# [priority]  — Inverted priority lane (lower number = earlier)
# ═════════════════════════════════════════════════════════════════════════════════
[priority]
priority_max_k = 1000                # upper bound for --priority <K> (guarantees the overflow bound in §5.4).
compaction_mode = "eager"            # "eager" (after every enqueue/dequeue) | "threshold"
compaction_threshold = 1024          # used only when compaction_mode = "threshold": compact when P_max exceeds this.
max_consecutive_priority = 8         # starvation guard: serve one Baseline entry after this many Priority dispatches.

# ═════════════════════════════════════════════════════════════════════════════════
# [phase2]  — Dispatch, focus lock, settle
# ═════════════════════════════════════════════════════════════════════════════════
[phase2]
dbus_call_timeout_ms = 250           # per D-Bus call from the coordinator (2A ack, Ping).
spawn_report_timeout_ms = 50         # 2B: max wait for all members to report SPAWNED before releasing the lock.
map_timeout_ms = 4000                # 2C: max wait for all members to map before the batch settles degraded.
settle_watchdog_ms = 250             # 2D: max settle duration; on expiry the extension force-uncloaks and releases.
configure_ack_timeout_ms = 120       # per-window wait for the client's resize ack before revealing anyway.
settle_frames = 1                    # compositor frames to wait after clamp for late sub-surface commits.
uncloak_ramp_ms = 20                 # opacity 0 -> 255 ramp (~1-2 frames at 60 Hz).
cloak_mode = "inert"                 # "inert" (opacity 0 + input-inert) | "opacity" (opacity only; fallback).
cloak_max_ms = 6000                  # absolute cloak ceiling (I8); must exceed map_timeout_ms + settle_watchdog_ms.
degrade_grace_ms = 150               # after coordinator loss, extension settles mapped members autonomously.
cli_wait_for_settle = true           # CLI exits at SETTLED (G1). false == --no-wait for every command.

# ═════════════════════════════════════════════════════════════════════════════════
# [matching]  — claim identity & TTL
# ═════════════════════════════════════════════════════════════════════════════════
[matching]
claim_ttl_ms = 5000                  # I9; MUST be >= map_timeout_ms.
match_window_types = ["NORMAL"]      # ignore splash/dialog/tooltip windows when fulfilling claims.
pid_opaque_classes = [               # classes whose window owner PID != spawned PID (server/single-instance model).
  "org.gnome.Terminal", "org.gnome.Nautilus",
]
learn_pid_opaque_classes = true      # add a class at runtime after one failed PID match.

# ═════════════════════════════════════════════════════════════════════════════════
# [bypass]  — --bypass-fifo / out-of-band execution and focus soft-yield
# ═════════════════════════════════════════════════════════════════════════════════
[bypass]
bypass_yield_ms = 100                # soft-yield barrier: suppress in-flight groups' focus calls for this long.
bypass_yield_max_ms = 1000           # cap on rolling extension of the barrier.
bypass_restore_deferred_focus = true # restore a group's suppressed focus if the bypass window vanished.
bypass_rate_limit_per_sec = 10       # token bucket per canonical class; excess => raw spawn without placement.

# ═════════════════════════════════════════════════════════════════════════════════
# [placement]  — geometry rules used at Settle
# ═════════════════════════════════════════════════════════════════════════════════
[placement]
use_frame_rect = true                # place by frame rect (excludes CSD shadow), not buffer rect.
fractional_scale_snap = true         # snap shared edges so physical rounding is identical on both tiles.
group_focus_target = "leader"        # "leader" | "first" | "last" | "none"; falls back to first member if leaderless.
fallback_monitor = "primary"         # used when the target monitor disappears before Settle.

# ═════════════════════════════════════════════════════════════════════════════════
# [recovery]  — failure policy (I7)
# ═════════════════════════════════════════════════════════════════════════════════
[recovery]
coordinator_failure_policy = "fail_open"       # "fail_open" (launch raw) | "fail_closed" (exit 3)
extension_unavailable_policy = "fail_open"     # "fail_open" (launch raw) | "fail_closed" (exit 4)
coordinator_connect_timeout_ms = 200
heartbeat_interval_ms = 500                    # extension -> coordinator Ping cadence.
heartbeat_miss_limit = 3                       # consecutive misses => treat coordinator as hung/dead.
on_resume_policy = "flush_degraded"            # "flush_degraded" | "abort" | "continue"
legacy_arm_lock = true                         # allow flock(2) fallback on /run/user/$UID/spawn-at-arm.lock (L1).

# ═════════════════════════════════════════════════════════════════════════════════
# [paths]
# ═════════════════════════════════════════════════════════════════════════════════
[paths]
socket = "/run/user/$UID/spawn-at/coordinator.sock"   # directory 0700, socket 0600.
arm_lock = "/run/user/$UID/spawn-at-arm.lock"

# ═════════════════════════════════════════════════════════════════════════════════
# [logging]
# ═════════════════════════════════════════════════════════════════════════════════
[logging]
level = "info"                       # "error" | "warn" | "info" | "debug" | "trace"
trace_ring_entries = 4096
dump_trace_on_abort = true
```

---

*End of blueprint.*
