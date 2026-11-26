---
summary: >-
  Classroom statistics append-only JSONL schema (schema_version 1) streamed
  directly to ephemeral SSD (`LOG_DIR`, default `/tmp/trafficlights-logs`):
  file lifecycle, event specifications (including optional `collect_names` and
  `actor` attribution), server invariants, Python plotting recipes (pandas +
  matplotlib), and future SQLite/Turso archival mapping.
---

# Classroom statistics JSONL schema

## Overview

Each classroom session writes an **append-only** newline-delimited JSON
(`.jsonl`) log directly to the container's ephemeral SSD. Streaming transition
events straight to disk respects the host's **88% RAM budget (`R15`)** by
keeping active session state under 1 KB in memory and avoiding an embedded
Turso/SQLite engine (~13.6 MiB RSS overhead).

- **Encoding:** UTF-8, LF-delimited (`\n`), one JSON object per line.
- **Location:** `LOG_DIR/<session_id>.jsonl` (`LOG_DIR` defaults to
  `/tmp/trafficlights-logs`).
- **Lifecycle:**
  1. Created when the session starts (`session_start` is written as line 1).
  2. Each subsequent event is serialized as one line, appended immediately, and
     flushed to disk.
  3. The session creator can download the live or finalized file at any time
     via `GET /api/sessions/{id}/log` (streamed in O(1) RAM with
     `Authorization: Bearer <creator_token>`).
  4. When the session ends, `session_end` is appended and flushed.
  5. After a post-end download window (`LOG_RETENTION_SECONDS`, default `600`
     seconds), the temporary `.jsonl` file is deleted to keep the ephemeral SSD
     clean.

## Event schema (`schema_version: 1`)

Every line is a self-contained JSON object with:

- `event` (**string**) — event discriminator (`session_start`,
  `receiver_status`, `state_change`, `range_change`, `viewer_count`,
  `session_end`).
- `ts_ms` (**integer**) — UTC timestamp in milliseconds since Unix epoch.
- `elapsed_ms` (**integer**) — milliseconds elapsed since `session_start.ts_ms`
  (`0` on `session_start`, non-decreasing thereafter).

All event types are defined in `crates/proto` and serialized by `crates/server`.
The viewer-name keys (`collect_names` on `session_start` and `actor` on
`state_change`) are backward-compatible optional additions to
`schema_version: 1`: they are written last and omitted when unused so sessions
without viewer names produce byte-identical output and existing readers that
ignore unknown keys continue to work.

### `session_start`

```json
{"event":"session_start","schema_version":1,"ts_ms":1764086400000,"elapsed_ms":0,"session_id":"0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60","name":"Studio 3B — Week 4","range":{"start":1,"end":60}}
```

When the session collects viewer names:

```json
{"event":"session_start","schema_version":1,"ts_ms":1764086400000,"elapsed_ms":0,"session_id":"0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60","name":"Studio 3B — Week 4","range":{"start":1,"end":60},"collect_names":true}
```

- `schema_version` (**integer**) — always `1` for this schema.
- `session_id` (**string**) — session identifier (UUIDv7).
- `name` (**string**) — human-readable session name.
- `range` (**object**) — initial inclusive table range `{"start": int, "end": int}`.
- `collect_names` (**boolean**, optional, omitted when `false`) — `true` when
  viewers must identify before changing tables, so web-app `state_change`
  records include `actor`. Fixed for the session's lifetime.

### `receiver_status`

```json
{"event":"receiver_status","ts_ms":1764086401200,"elapsed_ms":1200,"connected":true,"firmware":{"kind":"new","mode":"receiver","version":"draft-1"}}
```

- `connected` (**boolean**) — `true` when the creator's browser Web Serial
  bridge is connected to the receiver; `false` when disconnected.
- `firmware` (**object**, optional, omitted when absent) —
  `{"kind": "new" | "legacy", "mode"?: "receiver" | "station", "version"?: string}`.

### `state_change`

```json
{"event":"state_change","ts_ms":1764086415500,"elapsed_ms":15500,"table":12,"prev":"green","next":"red","source":"student_radio","in_range":true}
```

For a web-app change in a session with `"collect_names": true`:

```json
{"event":"state_change","ts_ms":1764086430000,"elapsed_ms":30000,"table":12,"prev":"red","next":"green","source":"teacher_click","in_range":true,"actor":"Alice"}
```

- `table` (**integer**, `0`–`9999`) — station/table ID (`0` indicates an unset
  or misnumbered station).
- `prev` (**string** or `null`) — previous color (`"green"`, `"orange"`,
  `"red"`), or `null` when an out-of-range table is heard for the first time
  (in-range tables start at `"green"`).
- `next` (**string**) — new color (`"green"`, `"orange"`, `"red"`).
- `source` (**string**) — input channel of the transition:
  - `"student_radio"` — reported by the physical station over radio/serial
    (note: TAs and teachers often clear a light by pressing the physical button
    at the table rather than clicking the web app, so `"student_radio"` cannot
    be treated as "student self-resolved without TA help").
  - `"teacher_click"` — changed by clicking a single table tile in the web app.
  - `"teacher_reset_all"` — cleared by the creator's **Reset All To Green**
    action.
  - `"range_reset"` — emitted if a range change resets state.
- `in_range` (**boolean**) — whether `table` fell inside the active
  `start..=end` range at `ts_ms`.
- `actor` (**string**, optional, omitted when absent) — in sessions with
  `"collect_names": true`, names who made a web-app change (`"teacher_click"`,
  `"teacher_reset_all"`, or `"range_reset"`): either the viewer's normalized
  name (1–40 Unicode scalar values, unique per session) or `"host"` for the
  session creator. Always omitted for `"student_radio"` events and in sessions
  where `collect_names` is `false`.

### `range_change`

```json
{"event":"range_change","ts_ms":1764086520000,"elapsed_ms":120000,"prev_range":{"start":1,"end":60},"next_range":{"start":1,"end":30}}
```

- `prev_range`, `next_range` (**object**) — `{"start": int, "end": int}`
  defining the previous and new inclusive table-ID ranges.

### `viewer_count`

```json
{"event":"viewer_count","ts_ms":1764086525000,"elapsed_ms":125000,"viewers":5}
```

- `viewers` (**integer**) — total connected viewer clients (excluding the
  creator) after the join/leave change.

### `session_end`

```json
{"event":"session_end","ts_ms":1764090000000,"elapsed_ms":3600000,"reason":"manual"}
```

- `reason` (**string**) — why the session closed: `"manual"` (creator clicked
  **End session**), `"grace_timeout"` (disconnect grace window expired), or
  `"shutdown"` (server received `SIGTERM`/`SIGINT`).

## Invariants enforced by `crates/proto` and `crates/server`

1. **Header first:** line 1 is always `session_start` with
   `"schema_version": 1` and `"elapsed_ms": 0`.
2. **Monotonic timestamps:** `ts_ms` and `elapsed_ms` never decrease within a
   session file.
3. **Transition-only logging:** `state_change` is emitted only when a table's
   color actually changes (or on an explicit reset), keeping repeated 4–6 s
   radio heartbeats out of the log.
4. **Out-of-range auditability:** `in_range: false` events (such as an unset
   station with `table: 0`) are logged for debugging while being trivially
   filterable out of classroom averages.
5. **Offline gap visibility:** `receiver_status` events mark every serial or
   creator WebSocket disconnect/reconnect so plots can shade unreliable
   intervals.
6. **Atomic failure semantics:** failed commands while the receiver is
   disconnected never mutate table state and never emit synthetic
   `state_change` events.
7. **Viewer-name attribution and privacy:** when `"collect_names": true`,
   viewers cannot submit `set_table` until identified, viewer names are kept
   per connection and never broadcast over WebSockets, `"host"` is reserved
   case-insensitively for the session creator, and `actor` appears only on
   web-app transitions (`teacher_click`, `teacher_reset_all`, `range_reset`),
   never on `student_radio` or in non-collecting sessions.

## Offline plotting tool and Python recipes

The dedicated offline analytics and plotting CLI lives in the sibling repository
`../trafficlights_analytics`
([NB-TUDelft/trafficlights_analytics](https://github.com/NB-TUDelft/trafficlights_analytics),
`AGPL-3.0-or-later`). Built on Matplotlib + NumPy, it parses `schema_version: 1`
`.jsonl` files, merges physical micro:bit button-stepping transitions into
single help episodes, computes both active-table and configured-range average
states, flags `in_range: false` stations as hardware diagnostics, and emits a
combined `00_dashboard.pdf`/`.svg` plus individual section PNGs (`01`–`05`).

The standalone `pandas` + `matplotlib` snippets below remain as minimal
reference recipes (`pip install pandas matplotlib`):

```python
import json
import matplotlib.pyplot as plt
import pandas as pd

with open("stats.jsonl", "r", encoding="utf-8") as fh:
    records = [json.loads(line) for line in fh if line.strip()]

df = pd.DataFrame(records)
df["ts"] = pd.to_datetime(df["ts_ms"], unit="ms", utc=True)

start_row = df[df["event"] == "session_start"].iloc[0]
end_rows = df[df["event"] == "session_end"]
end_ts = end_rows.iloc[0]["ts"] if not end_rows.empty else df["ts"].max()

state_events = df[df["event"] == "state_change"].sort_values("ts_ms")
range_events = df[df["event"] == "range_change"].sort_values("ts_ms")

# ------------------------------------------------------------
# 1. Active light counts (green / orange / red) over time
# ------------------------------------------------------------
current_range = (start_row["range"]["start"], start_row["range"]["end"])
range_iter = iter(range_events.itertuples())
next_range = next(range_iter, None)

colors: dict[int, str] = {}
snapshots = [
    {
        "ts": start_row["ts"],
        "green": current_range[1] - current_range[0] + 1,
        "orange": 0,
        "red": 0,
    }
]

for row in state_events.itertuples():
    while next_range and next_range.ts_ms <= row.ts_ms:
        current_range = (
            next_range.next_range["start"],
            next_range.next_range["end"],
        )
        next_range = next(range_iter, None)
    if row.in_range:
        colors[int(row.table)] = row.next
    counts = {"green": 0, "orange": 0, "red": 0}
    for tbl in range(current_range[0], current_range[1] + 1):
        counts[colors.get(tbl, "green")] += 1
    snapshots.append({"ts": row.ts, **counts})

light_df = pd.DataFrame(snapshots).set_index("ts")
ax = light_df.plot(
    figsize=(10, 4),
    drawstyle="steps-post",
    color=["#57d86b", "#ffa534", "#ff4d4d"],
)
# Shade offline periods from receiver_status
offline = df[(df["event"] == "receiver_status") & (df["connected"] == False)]
for _, r in offline.iterrows():
    later = df[
        (df["event"] == "receiver_status")
        & (df["connected"] == True)
        & (df["ts_ms"] > r["ts_ms"])
    ]
    gap_end = later.iloc[0]["ts"] if not later.empty else end_ts
    ax.axvspan(r["ts"], gap_end, color="gray", alpha=0.3)

ax.set_title("Active light counts over time")
ax.set_xlabel("Time (UTC)")
ax.set_ylabel("Tables")
plt.tight_layout()
plt.show()

# ------------------------------------------------------------
# 2. Per-table timeline heatmap (in-range tables only)
# ------------------------------------------------------------
in_range_states = state_events[state_events["in_range"] == True]
pivot = (
    in_range_states.pivot_table(
        index="table", columns="ts", values="next", aggfunc="last"
    )
    .ffill(axis=1)
    .fillna("green")
)
color_map = {"green": 0, "orange": 1, "red": 2}
heat = pivot.replace(color_map).astype(int)

plt.figure(figsize=(12, 6))
plt.imshow(heat, aspect="auto", cmap="RdYlGn_r", vmin=0, vmax=2, interpolation="none")
plt.colorbar(label="Color (0=green, 1=orange, 2=red)")
plt.title("Per-table color timeline")
plt.xlabel("Event index")
plt.ylabel("Table ID")
plt.tight_layout()
plt.show()

# ------------------------------------------------------------
# 3. Average class state over time (0 = all green, 2 = all red)
# ------------------------------------------------------------
avg_series = heat.mean(axis=0)
plt.figure(figsize=(10, 3))
plt.plot(avg_series.index, avg_series.values, drawstyle="steps-post")
plt.ylim(0, 2)
plt.title("Average class state over time")
plt.xlabel("Time (UTC)")
plt.ylabel("Mean state (0-2)")
plt.tight_layout()
plt.show()

# ------------------------------------------------------------
# 4. Help wait-time distribution (by peak color; source cannot distinguish
#    who pressed the physical button at the table)
# ------------------------------------------------------------
need_help = in_range_states[in_range_states["next"].isin(["orange", "red"])]
resolved = in_range_states[in_range_states["next"] == "green"]

wait_s: list[float] = []
for row in need_help.itertuples():
    later = resolved[
        (resolved["table"] == row.table) & (resolved["ts_ms"] > row.ts_ms)
    ]
    if not later.empty:
        first = later.iloc[0]
        wait_s.append((first["ts"] - row.ts).total_seconds())

plt.figure(figsize=(8, 4))
plt.hist(wait_s, bins=20, edgecolor="black")
plt.title("Time from orange/red until green")
plt.xlabel("Seconds until cleared")
plt.ylabel("Count")
plt.tight_layout()
plt.show()

# ------------------------------------------------------------
# 5. Web-app resolutions by actor (sessions with collect_names: true)
# ------------------------------------------------------------
if "actor" in in_range_states.columns:
    by_actor = (
        in_range_states[
            (in_range_states["source"] == "teacher_click")
            & (in_range_states["next"] == "green")
        ]
        .dropna(subset=["actor"])["actor"]
        .value_counts()
    )
    if not by_actor.empty:
        by_actor.plot(kind="bar", figsize=(8, 4))
        plt.title("Table resolutions by actor")
        plt.xlabel("Actor")
        plt.ylabel("Resolved tables")
        plt.tight_layout()
        plt.show()
```

## Future archival to SQLite/Turso

When long-term multi-session archival is added later, `schema_version: 1`
provides an explicit import boundary. An offline ETL script can load any
`.jsonl` file into a relational table without changing live producers:

```sql
CREATE TABLE classroom_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    ts_ms BIGINT NOT NULL,
    elapsed_ms BIGINT NOT NULL,
    event TEXT NOT NULL,
    name TEXT,
    collect_names BOOLEAN,
    connected BOOLEAN,
    firmware_kind TEXT,
    firmware_mode TEXT,
    firmware_version TEXT,
    table_id INTEGER,
    prev_color TEXT,
    next_color TEXT,
    source TEXT,
    in_range BOOLEAN,
    actor TEXT,
    range_start INTEGER,
    range_end INTEGER,
    viewers INTEGER,
    reason TEXT
);
```
