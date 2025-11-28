# COD Matchmaking Whitepaper Implementation Roadmap

## Overview

This roadmap guides the implementation of a full agent-based matchmaking simulation that matches the mathematical model described in `cod_matchmaking_model.md`. The current codebase already implements a substantial portion of the whitepaper (approximately Stage 1-2), and this document breaks down the remaining work into **vertical slices** that can be implemented incrementally.

**📝 Important**: See `NOTES.md` for implementation learnings, common mistakes, and gotchas discovered during development.

### Progress Summary

**Completed Slices**:
- ✅ **Slice A: Parties & Multi-Player Search Objects**
  - Full party system integrated into matchmaking
  - WASM frontend integration complete
  - Party metrics and visualizations added
  - Automatic party generation via config parameter
- ✅ **Slice B: Matchmaking Constraints & Backoff Refinement**
  - Fixed critical units mismatch bug (ticks → seconds)
  - Fixed skill range check to match whitepaper §3.3 exactly
  - Added optional debug logging behind feature flag
  - All backoff formulas verified correct
- ✅ **Slice C: Team Balancing & Blowout Modeling**
  - Exact team balancing for small playlists (6v6) using Karmarkar-Karp partitioning
  - Enhanced blowout detection with severity classification (Mild, Moderate, Severe)
  - Configurable win probability logistic with gamma parameter
  - Per-playlist blowout rate tracking
  - Team skill difference distribution tracking
  - Frontend charts for blowout metrics and severity distribution

**Remaining Slices**: D, E, F, G, H (optional)

### Relationship to Whitepaper

The whitepaper (`cod_matchmaking_model.md`) describes:
- **Section 2**: State & variables (players, DCs, playlists, skill, search objects)
- **Section 3**: High-fidelity matchmaking process (seed+greedy, feasibility, quality scoring, team balancing)
- **Section 4-5**: Reduced/aggregate models for scale (optional, later phase)
- **Section 6**: Treatment of each CoD variable (connection, skill, input, platform, etc.)
- **Section 7**: Concrete build order (Stage 0-4)

This roadmap focuses on completing the **agent-based model** (Stages 1-3) and preparing for the aggregate model (Stage 4, optional).

### Current Implementation Status

The Rust/WASM engine (`src/`) already implements:
- ✅ Player state machine (OFFLINE → IN_LOBBY → SEARCHING → IN_MATCH)
- ✅ Data centers with ping modeling and backoff
- ✅ Skill system (raw skill, percentiles, buckets)
- ✅ Search objects and seed+greedy matchmaking
- ✅ Feasibility constraints (playlist, size, skill similarity/disparity, DC intersection, server capacity)
- ✅ Quality scoring (ping, skill balance, wait time)
- ✅ Team balancing (exact partitioning for 6v6, snake draft for large playlists)
- ✅ Match outcomes with configurable logistic and blowout severity classification
- ✅ Basic retention/continuation logic
- ✅ Per-bucket statistics
- ✅ Blowout severity tracking and per-playlist metrics

The React frontend (`web/src/`) provides:
- ✅ Real-time visualization (charts, histograms, bucket stats)
- ✅ Parameter sweeps and experiments
- ✅ Configuration controls
- ✅ Full WASM integration (Rust simulation engine running in browser)
- ✅ Party metrics and visualizations
- ✅ Blowout rate by playlist and severity distribution charts
- ✅ Team balancing configuration controls

---

## Current State vs Whitepaper Mapping

| Topic | Whitepaper Section | Current Implementation | Gap / To-Do |
|-------|-------------------|------------------------|-------------|
| **Player State Machine** | §2.5 | `PlayerState` enum, 4-state loop | ✅ Complete |
| **Player Attributes** | §2.2 | `Player` struct (location, platform, input, skill, playlists) | ✅ Complete |
| **DC & Ping Model** | §2.3 | `DataCenter`, `dc_pings`, `acceptable_dcs()` with backoff | ✅ Complete |
| **Skill System** | §2.4 | Raw skill, percentile, buckets | ⚠️ Static (no evolution) |
| **Search Objects** | §2.7 | `SearchObject` struct | ✅ Complete (supports solo and parties) |
| **Distance Metric** | §3.1 | `calculate_distance()` with weights | ✅ Complete |
| **Feasibility Checks** | §3.3 | `check_feasibility()` implements 6 constraints | ✅ Complete (units fixed, skill range check corrected) |
| **Quality Score** | §3.4 | `calculate_quality()` with 3 components | ✅ Complete |
| **Team Balancing** | §3.6 | `balance_teams()` with exact partitioning for 6v6 | ✅ Complete (exact for small, snake draft for large) |
| **Match Outcomes** | §3.7 | `determine_outcome()` with configurable logistic and blowout severity | ⚠️ No performance model/KPM |
| **Skill Evolution** | §3.7 | None | ❌ Missing |
| **Retention Model** | §3.8 | Inline continuation probability | ⚠️ Ad-hoc (not formal logistic) |
| **Parties** | §2.4, §2.7 | `Party` struct with full integration | ✅ Complete |
| **Region Graph** | §2.3, §6.1 | Regions as strings, no adjacency | ⚠️ Implicit (needs explicit) |
| **Under-full Lobbies** | §6.8 | Exact size match only | ⚠️ Missing |
| **Aggregate Model** | §5 | None | ❌ Optional Phase |

**Legend**: ✅ Complete | ⚠️ Partial/Needs Refinement | ❌ Missing

---

## Vertical Slices

Each vertical slice is a self-contained feature that touches engine, metrics, and optionally frontend. Slices can be implemented independently, but some have dependencies (noted below).

### Slice A: Parties & Multi-Player Search Objects ✅ **COMPLETE**

**Whitepaper References**: §2.4 (party aggregates), §2.7 (search objects), §3.6 (team balancing with parties)

**Status**: ✅ **Completed**

**Goals**:
- ✅ Enable players to form parties and search together
- ✅ Build `SearchObject`s from parties (not just solo players)
- ✅ Maintain party integrity during matchmaking (no splitting parties across teams)
- ✅ Compute party-level skill aggregates (\(\bar{s}_P\), \(\Delta s_P\), \(\bar{\pi}_P\), \(\Delta\pi_P\))

**Engine Work**:
- ✅ **`src/types.rs`**:
  - Extended `Party` struct: added `preferred_playlists: HashSet<Playlist>`, `platforms: HashMap<Platform, usize>`, `input_devices: HashMap<InputDevice, usize>`, `avg_location: Location`, `avg_skill_percentile`, `skill_percentile_disparity`
  - Added methods: `Party::from_players(players: &[&Player]) -> Party`, `Party::update_aggregates()`, `Party::to_search_object()`
- ✅ **`src/simulation.rs`**:
  - Added `parties: HashMap<usize, Party>` to `Simulation`
  - Implemented `create_party(player_ids: Vec<usize>) -> Result<usize, String>`
  - Implemented `join_party(party_id: usize, player_id: usize)`, `leave_party(party_id: usize, player_id: usize)`, `disband_party(party_id: usize)`
  - Modified `start_search()`: if player has `party_id`, create `SearchObject` from party; otherwise solo
  - Updated `SearchObject` creation to compute aggregates from party members
  - Added automatic party generation in `generate_population()` based on `party_player_fraction` config
- ✅ **`src/matchmaker.rs`**:
  - Updated `balance_teams()` to respect party boundaries (no splitting parties)
  - Team balancing uses party-aggregated skills when assigning teams
- ✅ **`src/lib.rs`**:
  - Exposed party management methods via WASM bindings
  - Added `get_parties()`, `get_party_members()`, `get_lobby_players()` for UI integration

**Frontend Work**:
- ✅ Full WASM integration replacing JavaScript simulation engine
- ✅ Party metrics displayed in Overview tab (party count, avg size, match rates, search times)
- ✅ Search queue visualization showing solo vs party searches
- ✅ Party size distribution and party vs solo search time comparison charts
- ✅ Config parameter: `party_player_fraction` (0.0-1.0) to control automatic party generation
- ✅ Removed manual party creation UI in favor of automatic generation based on config

**Metrics & Experiments**:
- ✅ Track: average party size, party match rate vs solo match rate, skill disparity within parties
- ✅ Party search times vs solo search times tracked and displayed
- ✅ Party size distribution visualization

**Enhancements Beyond Original Plan**:
- Added `party_player_fraction` config parameter (default 0.5) to automatically generate parties during population creation
- This allows controlled solo vs party mix without manual intervention, aligning with whitepaper's "50% parties of size 2-4" experiment scenario

**Dependencies**: None (was first slice)

---

### Slice B: Matchmaking Constraints & Backoff Refinement

**Whitepaper References**: §2.3 (DC backoff), §2.7 (skill backoff), §3.3 (feasibility)

**Status**: ✅ **COMPLETE**

**Critical Issues Fixed**:
1. ✅ **Units Mismatch (BUG FIXED)**: `SearchObject::wait_time()` now returns seconds by accepting `tick_interval` parameter. All 8 call sites updated.
2. ✅ **Skill Range Check (BUG FIXED)**: Replaced incorrect implementation with correct whitepaper §3.3 formula: \([\pi_{\min}(M), \pi_{\max}(M)] \subseteq [\ell_j(t), u_j(t)]\) for all searches j.
3. ✅ **Backoff Formulas**: Verified correct - formulas match whitepaper exactly.

**Goals**:
- ✅ Fix units mismatch (ticks → seconds) in `wait_time()` and all backoff calls
- ✅ Fix skill range check to match whitepaper §3.3 exactly
- ✅ Add debug logging for feasibility failures (optional, behind feature flag)
- **Defer**: Under-full lobby support (optional, per whitepaper §6.8 - only needed for extremely sparse populations)

**Engine Work**:
- ✅ **`src/types.rs`**:
  - ✅ Backoff methods already match whitepaper formulas (no changes needed):
    - \(f_{\text{conn}}(w) = \min(\delta_{\text{init}} + \delta_{\text{rate}} \cdot w, \delta_{\text{max}})\)
    - \(f_{\text{skill}}(w) = \min(\sigma_{\text{init}} + \sigma_{\text{rate}} \cdot w, \sigma_{\text{max}})\)
  - ✅ Fixed `SearchObject::wait_time()`: now returns **seconds** by multiplying ticks by `tick_interval` parameter
- ✅ **`src/matchmaker.rs`**:
  - ✅ Updated all 8 `wait_time()` call sites to pass `tick_interval` parameter
  - ✅ Fixed `check_feasibility()` skill similarity check:
    - For each search j, compute \(\ell_j(t) = \bar{\pi}_j - f_{\text{skill}}(w_j)\) and \(u_j(t) = \bar{\pi}_j + f_{\text{skill}}(w_j)\)
    - Verify \([\pi_{\min}(M), \pi_{\max}(M)] \subseteq [\ell_j(t), u_j(t)]\) for all j
    - Replaced incorrect `skill_range > allowed_range * 2.0` check
  - ✅ Added debug logging (behind `#[cfg(feature = "debug")]`) that records why feasibility checks fail
  - ✅ Fixed skill disparity check to use correct variable name
- ✅ **`src/simulation.rs`**:
  - ✅ Verified `tick_interval` is accessible where `wait_time()` is called
  - ✅ Audited all tick ↔ seconds conversions for consistency (all correct)
- ✅ **`src/lib.rs`**:
  - ✅ Updated WASM binding to use new `wait_time()` signature
- ✅ **`Cargo.toml`**:
  - ✅ Added optional `debug` feature flag

**Frontend Work**:
- ⚠️ Debug panel deferred (debug logging available in console when feature enabled)

**Metrics & Experiments**:
- ✅ Added unit tests: `test_wait_time_converts_ticks_to_seconds`, `test_backoff_formulas`, `test_backoff_with_seconds`, `test_skill_range_check_correct`
- ✅ Validated: backoff curves match expected formulas when wait_time is in seconds
- ✅ Validated: skill range constraints work correctly with fixed implementation

**Optional/Future Work** (defer to later slice if needed):
- Under-full lobby support: Add config `allow_underfull_lobbies: bool`, `underfull_threshold: f64`, `underfull_min_wait_seconds: f64`
- Frontend config sliders for under-full lobby parameters
- Frontend debug panel to display feasibility failure reasons

**Dependencies**: None (can be parallel with Slice A)

---

### Slice C: Team Balancing & Blowout Modeling ✅ **COMPLETE**

**Whitepaper References**: §3.6 (team balancing), §3.7 (outcomes, blowouts)

**Status**: ✅ **Completed**

**Goals**:
- ✅ Improve team balancing to better approximate Karmarkar-Karp partitioning
- ✅ Enhance blowout detection with more nuanced metrics
- ✅ Track blowout severity/severity buckets

**Engine Work**:
- ✅ **`src/types.rs`**:
  - Added `BlowoutSeverity` enum: `{ Mild, Moderate, Severe }`
  - Extended `Match`: `expected_score_differential: f64`, `win_probability_imbalance: f64`, `blowout_severity: Option<BlowoutSeverity>`
  - Extended `MatchmakingConfig`: `use_exact_team_balancing: bool`, `gamma: f64`, blowout detection coefficients and thresholds
  - Extended `SimulationStats`: `blowout_severity_counts`, `per_playlist_blowout_rate`, `team_skill_difference_samples`, per-playlist tracking fields
- ✅ **`src/matchmaker.rs`**:
  - Refactored `balance_teams()`:
    - Implemented exact partitioning for small playlists (6v6) using recursive backtracking
    - Minimizes `|sum(skills_team1) - sum(skills_team2)|` while respecting party boundaries
    - Falls back to snake draft for large playlists or if exact partitioning fails
    - Always ensures parties stay intact (no splitting)
  - Added `exact_partition_teams()` and `exact_partition_recursive()` helper methods
- ✅ **`src/simulation.rs`**:
  - Enhanced `determine_outcome()`:
    - Uses configurable logistic: \(P(A \text{ wins}) = \sigma(\gamma (S_A - S_B))\) with configurable \(\gamma\)
    - Computes `win_probability_imbalance` and `expected_score_differential`
    - Refactored blowout detection using configurable coefficients for skill difference vs win-probability imbalance
    - Assigns `blowout_severity` based on configurable thresholds
  - Updated `create_matches()` to calculate and store new match fields
  - Updated `process_match_completions()` to track blowout severity and per-playlist stats
  - Updated `update_stats()` to calculate per-playlist blowout rates

**Frontend Work**:
- ✅ Added config sliders:
  - `useExactTeamBalancing` (checkbox)
  - `gamma` (0.5-5.0)
  - Blowout coefficients and thresholds (6 sliders)
- ✅ Added charts:
  - Blowout Rate by Playlist (bar chart)
  - Blowout Severity Distribution (bar chart with color coding)
  - Team Skill Difference Distribution (histogram)
- ✅ Updated `defaultConfig` and `convertConfigToRust()` with new fields
- ✅ Updated stats parsing to include new metrics

**Metrics & Experiments**:
- ✅ Track: team skill difference distribution, blowout rate by playlist, blowout severity breakdown
- ✅ Experiment ready: Compare blowout rates with exact vs heuristic team balancing

**Dependencies**: Slice A (parties) recommended but not required

---

### Slice D: Performance Model & Skill Evolution

**Whitepaper References**: §2.4 (skill), §3.7 (performance, skill update), §6.4 (skill evolution)

**Goals**:
- Add per-match performance model (KPM/SPM or performance index)
- Implement skill update rule based on performance vs expectation
- Track skill distribution evolution over time

**Engine Work**:
- **`src/types.rs`**:
  - Add to `Player`: `recent_performance: Vec<f64>` (performance indices from recent matches)
  - Add to `Match`: `player_performances: HashMap<usize, f64>` (performance index per player)
  - Add to `MatchmakingConfig`: `skill_learning_rate: f64` (α in update rule), `performance_noise_std: f64`
- **`src/simulation.rs`**:
  - Add function `generate_performance(player: &Player, lobby_avg_skill: f64, playlist: Playlist, rng: &mut impl Rng) -> f64`:
    - Base performance = \(f_{\text{perf}}(s_i, \bar{s}_{\text{lobby}}, m)\)
    - Add noise: \(\epsilon_i \sim \mathcal{N}(0, \sigma^2)\)
    - Return normalized performance index (0-1 scale)
  - After match completion, compute performance for each player and store in `Match.player_performances`
  - Implement skill update: \(s_i^+ = s_i^- + \alpha (\hat{y}_i - \mathbb{E}[Y_i \mid s_i, \text{lobby}])\)
    - \(\hat{y}_i\) = normalized performance vs lobby average
    - \(\mathbb{E}[Y_i]\) = expected performance (function of skill and lobby context)
  - After batches of matches (every N ticks or every M matches), call `update_skill_percentiles()` to recompute percentiles and buckets
  - Add to `SimulationStats`: `skill_distribution_over_time: Vec<(u64, Vec<(usize, f64)>)>` (time series of bucket means)

**Frontend Work**:
- Add chart: skill distribution evolution over time (animated or time slider)
- Add toggle: "Static Skill" vs "Evolving Skill" mode
- Show performance indices in match details (optional)

**Metrics & Experiments**:
- Track: skill drift over time, performance distribution by skill bucket, skill update rate
- Experiment: Compare blowout rates and search times with static vs evolving skill

**Dependencies**: Slice C (team balancing) recommended for accurate performance context

---

### Slice E: Satisfaction, Continuation, and Retention Modeling

**Whitepaper References**: §3.8 (satisfaction, quit probability), §6.9 (KPIs)

**Goals**:
- Replace ad-hoc continuation logic with formal logistic model
- Define experience vector and parameterized retention function
- Track per-bucket retention metrics

**Engine Work**:
- **`src/types.rs`**:
  - Add struct `RetentionConfig`:
    ```rust
    pub struct RetentionConfig {
        pub theta_ping: f64,      // Coefficient for delta ping
        pub theta_search_time: f64,
        pub theta_blowout: f64,
        pub theta_win_rate: f64,
        pub theta_performance: f64,
        pub base_continue_prob: f64,  // Base probability (before penalties)
    }
    ```
  - Add to `Player`: `recent_experience: Vec<ExperienceVector>` (last N matches)
  - Add struct `ExperienceVector`:
    ```rust
    pub struct ExperienceVector {
        pub avg_delta_ping: f64,
        pub avg_search_time: f64,
        pub blowout_rate: f64,
        pub win_rate: f64,
        pub avg_performance: f64,
    }
    ```
- **`src/simulation.rs`**:
  - Add function `compute_continue_probability(player: &Player, config: &RetentionConfig) -> f64`:
    - Build experience vector from recent history
    - Compute: \(P(\text{continue}) = \sigma(\theta^T \mathbf{z}_i)\)
    - Return probability
  - Replace inline continuation logic in `process_match_completions()` with call to `compute_continue_probability()`
  - After each match, update `player.recent_experience`
  - Add to `SimulationStats`: `per_bucket_continue_rate: HashMap<usize, f64>`, `avg_matches_per_session: f64`, `session_length_distribution: Vec<usize>`

**Frontend Work**:
- Add config panel for retention model coefficients
- Add presets: "Ping-First", "Skill-First", "Lenient", "Strict"
- Add chart: continuation rate by skill bucket
- Add chart: average matches per session over time

**Metrics & Experiments**:
- Track: continuation rate by bucket, matches per session, effective population size (concurrent players)
- Experiment: Compare population health (total concurrent players) with different retention models

**Dependencies**: Slice D (performance model) recommended for complete experience vector

---

### Slice F: Region/DC Graph & Regional Metrics

**Whitepaper References**: §2.3 (DC connectivity), §2.6 (DCs), §4 (regions), §6.1 (regional behavior)

**Goals**:
- Make regions explicit (enum instead of strings)
- Define region adjacency graph
- Add region-aware backoff and tuning
- Track region-split metrics

**Engine Work**:
- **`src/types.rs`**:
  - Add enum `Region { NorthAmerica, Europe, AsiaPacific, SouthAmerica, Other }`
  - Add to `DataCenter`: `region: Region` (replace `String`)
  - Add to `Player`: `region: Region` (derived from location or explicit)
  - Add struct `RegionConfig`:
    ```rust
    pub struct RegionConfig {
        pub max_ping: f64,
        pub delta_ping_initial: f64,
        pub delta_ping_rate: f64,
        pub skill_similarity_initial: f64,
        // ... other per-region overrides
    }
    ```
  - Add to `MatchmakingConfig`: `region_configs: HashMap<Region, RegionConfig>` (optional overrides)
  - Add function `Region::adjacent_regions() -> Vec<Region>` (define adjacency graph)
- **`src/simulation.rs`**:
  - Update `init_default_data_centers()`: use `Region` enum
  - Update `generate_population()`: assign `player.region` based on location
  - In `Player::acceptable_dcs()`: use region-aware backoff (prefer best region, then adjacent regions as wait grows)
- **`src/matchmaker.rs`**:
  - When expanding acceptable DCs, prioritize: best region → adjacent regions → other regions
- **`src/simulation.rs`**:
  - Add to `SimulationStats`: `region_stats: HashMap<Region, RegionStats>`
  - Add struct `RegionStats`:
    ```rust
    pub struct RegionStats {
        pub player_count: usize,
        pub avg_search_time: f64,
        pub avg_delta_ping: f64,
        pub blowout_rate: f64,
        pub active_matches: usize,
    }
    ```

**Frontend Work**:
- Add region filter dropdown
- Add region-split charts (search time, delta ping, blowout rate by region)
- Add DC map visualization (optional, show DCs with capacity/usage)

**Metrics & Experiments**:
- Track: search times by region, cross-region match rate, delta ping by region
- Experiment: Compare behavior in low-population vs high-population regions

**Dependencies**: Slice B (backoff refinement) recommended for region-aware backoff

---

### Slice G: Frontend Experiment Runner & Visualizations

**Whitepaper References**: §7 (experiments), §6.9 (KPIs)

**Goals**:
- Enhance frontend to support all new metrics from slices A-F
- Build reusable experiment runner UI
- Add scenario preset system

**Engine Work**:
- **`src/lib.rs`**:
  - Ensure all new stats/metrics are exposed via WASM (region stats, retention metrics, skill evolution, etc.)
  - Add functions: `get_region_stats() -> String`, `get_retention_stats() -> String`, `get_skill_evolution() -> String`
- **`src/types.rs`**:
  - Add `ScenarioPreset` struct (JSON-serializable config + description)

**Frontend Work**:
- **`web/src/MatchmakingSimulator.jsx`**:
  - Add new charts:
    - Per-bucket retention/continuation rate
    - Skill distribution evolution (time slider)
    - Region-split metrics (search time, delta ping, blowout rate)
    - Blowout severity distribution
    - Performance distribution by skill bucket
  - Enhance experiment runner:
    - Support multi-parameter sweeps
    - Save/load experiment configs
    - Compare multiple configs side-by-side
  - Add scenario preset system:
    - Load/save JSON configs
    - Presets: "Tight SBMM", "Loose SBMM", "Ping-First", "Skill-First", "Low Population", "High Population"
  - Add region filter controls
  - Add DC map overlay (optional, show DC locations and usage)

**Metrics & Experiments**:
- All experiments from slices A-F should be runnable from UI
- Document canonical experiments in `EXPERIMENTS.md` (see Slice H)

**Dependencies**: Slices A-F (all metrics must be implemented first)

---

### Slice H (Optional): Aggregate / Reduced Model

**Whitepaper References**: §5 (aggregate model), §7 Stage 4

**Goals**:
- Implement bucketed/ODE-style model for massive scale
- Derive pairing kernel and throughput functions from micro-sim
- Validate aggregate model against agent-based model

**Engine Work**:
- **`src/aggregate.rs`** (new module):
  - Define bucket structure: \((r, m, b, k)\) where \(r\)=region, \(m\)=playlist, \(b\)=skill bucket, \(k\)=wait bin
  - State variables: \(S_{rmbk}(t)\), \(P_{rmb}(t)\), \(H_{rmb}(t)\)
  - Implement ODE update rules:
    - Arrivals: \(\lambda_{rmb}(t)\)
    - Aging between wait bins
    - Match throughput: \(\mu_{rmbk}(t)\)
    - Match completions: \(P_{rmb}(t) / \mathbb{E}[L_m]\)
  - Implement pairing kernel \(K_{bb'}\) (empirically fit from micro-sim or analytical approximation)
  - Implement throughput function: \(\nu_{rm}(t) = \min(S_{rm}(t) / N_m^{\text{req}}, \sum_d F_{d,m}(t))\)
- **`src/simulation.rs`**:
  - Add function `export_micro_data() -> AggregateTrainingData` (export samples for fitting)
- **`src/lib.rs`**:
  - Add `AggregateSimulation` struct and WASM bindings
  - Add function `run_aggregate_simulation(config, initial_state) -> AggregateResults`

**Frontend Work**:
- Add toggle: "Micro" vs "Aggregate" simulation mode
- Run same experiments in both modes and compare results
- Visualize pairing kernel \(K_{bb'}\) as heatmap

**Metrics & Experiments**:
- Validate: aggregate model reproduces micro-model outputs (search times, delta ping, blowouts) within acceptable error
- Experiment: Run long-term scenarios (months) with aggregate model

**Dependencies**: Slices A-F (need complete micro-model first to fit parameters)

---

## Implementation Phases

Phases group slices into logical execution order. Each phase produces working artifacts and can be validated independently.

### Phase 1: Core Matchmaking Fidelity

**Slices**: A (Parties) ✅ **COMPLETE** + B (Constraints/Backoff) ✅ **COMPLETE**

**Goal**: Complete the core matchmaking loop with parties and accurate constraints.

**Deliverables**:
- ✅ Parties fully integrated into search and matchmaking
- ✅ Backoff functions match whitepaper formulas exactly (verified with unit tests)
- ⚠️ Under-full lobby support (optional, deferred)
- ✅ Debug logging for feasibility failures (behind feature flag)

**Validation**:
- ✅ Run simulation with parties and verify: party integrity maintained, search times reasonable
- ✅ Verify backoff curves match expected formulas (unit tests added)
- ✅ Verify skill range constraints work correctly (unit tests added)
- ⚠️ Compare search times with/without under-full lobbies in low-population scenarios (deferred)

**Status**: Phase 1 complete. Both slices A and B implemented and tested.

**Estimated Effort**: 2-3 weeks (Slice A: ~1 week, Slice B: ~1 week)

---

### Phase 2: Match Quality & Outcomes

**Slices**: C (Team Balancing/Blowouts) ✅ **COMPLETE** + D (Performance/Skill Evolution)

**Goal**: Improve match quality prediction and enable dynamic skill evolution.

**Deliverables**:
- ✅ Exact team balancing for small playlists
- ✅ Enhanced blowout detection with severity
- Performance model and skill update rule
- Skill distribution evolution tracking

**Validation**:
- ✅ Compare blowout rates with exact vs heuristic balancing (ready for testing)
- Verify skill evolution: players improve/decline based on performance
- Track skill distribution stability over long runs

**Status**: Slice C complete. Slice D remaining.

**Estimated Effort**: 3-4 weeks (Slice C: ~1 week, Slice D: ~2-3 weeks)

---

### Phase 3: Player Behavior & Regional Analysis

**Slices**: E (Retention) + F (Regions)

**Goal**: Model player satisfaction and enable regional analysis.

**Deliverables**:
- Formal retention model with experience vector
- Region adjacency graph and region-aware backoff
- Per-region metrics and analysis
- Retention presets (ping-first, skill-first, etc.)

**Validation**:
- Compare population health (concurrent players) with different retention models
- Analyze regional differences: search times, delta ping, blowout rates
- Verify low-population regions can spill into adjacent regions

**Estimated Effort**: 2-3 weeks

---

### Phase 4: Frontend & Experimentation

**Slice**: G (Frontend Enhancements)

**Goal**: Make all new features accessible via UI and enable comprehensive experiments.

**Deliverables**:
- All new metrics visualized (retention, skill evolution, regions, blowouts)
- Enhanced experiment runner (multi-param sweeps, config comparison)
- Scenario preset system
- Region filters and DC map

**Validation**:
- Run all canonical experiments from `EXPERIMENTS.md` via UI
- Verify scenario presets reproduce expected behaviors
- Test experiment runner with various parameter combinations

**Estimated Effort**: 2-3 weeks

---

### Phase 5 (Optional): Aggregate Model

**Slice**: H (Aggregate/ODE Model)

**Goal**: Enable massive-scale simulations via reduced model.

**Deliverables**:
- Bucketed ODE model implementation
- Pairing kernel and throughput functions (fitted from micro-sim)
- Aggregate simulation driver
- Validation against micro-model

**Validation**:
- Run identical scenarios in micro and aggregate modes
- Compare outputs: search times, delta ping, blowouts, retention
- Verify aggregate model runs 100x+ faster for large populations

**Estimated Effort**: 4-6 weeks

---

## Experiment Catalog

This section documents canonical experiments that can be run once the relevant slices are implemented. Each experiment should be reproducible via the frontend experiment runner.

### Experiment 1: SBMM Strictness Sweep ✅ **READY**

**Dependencies**: Slices A ✅, B ✅, C ✅

**Parameters**: Vary `skill_similarity_initial` from 0.01 to 0.3

**Metrics to Track**:
- Search time (P50, P90, P99) by skill bucket
- Delta ping by skill bucket
- Blowout rate overall and by bucket (now includes severity breakdown)
- Skill disparity distribution
- Team skill difference distribution (new from Slice C)
- Per-playlist blowout rates (new from Slice C)

**Expected Results**:
- Tighter SBMM → longer search times, especially for extreme skill buckets
- Tighter SBMM → lower blowout rate, better skill matching
- Tradeoff: search time vs match quality
- Exact team balancing should reduce blowout rates compared to snake draft

**Config Preset**: `experiments/sbmm_strictness_sweep.json`

**Status**: All dependencies complete. Experiment can be run with enhanced metrics from Slice C.

---

### Experiment 2: Ping vs Skill Weight Tradeoff

**Dependencies**: Slices A, B

**Parameters**: Vary `weight_skill` from 0.1 to 0.7 (with `weight_geo` = 1.0 - `weight_skill`)

**Metrics to Track**:
- Average delta ping
- Average search time
- Skill disparity
- Blowout rate

**Expected Results**:
- Higher skill weight → better skill matching, worse ping
- Higher geo weight → better ping, worse skill matching
- Optimal point depends on population density

**Config Preset**: `experiments/ping_vs_skill_tradeoff.json`

---

### Experiment 3: Retention Model Comparison

**Dependencies**: Slices E, D

**Parameters**: Compare retention presets: "Ping-First", "Skill-First", "Lenient", "Strict"

**Metrics to Track**:
- Effective population size (concurrent players) over time
- Average matches per session
- Continuation rate by skill bucket
- Churn rate

**Expected Results**:
- Ping-First → higher retention for low-ping players, lower for high-ping
- Skill-First → higher retention for mid-skill players
- Lenient → higher overall retention but more blowouts
- Strict → lower retention but better match quality

**Config Preset**: `experiments/retention_model_comparison.json`

---

### Experiment 4: Regional Population Effects

**Dependencies**: Slices F, B

**Parameters**: Vary regional population weights (e.g., NA: 0.7, EU: 0.2, APAC: 0.1 vs balanced 0.33 each)

**Metrics to Track**:
- Search time by region
- Delta ping by region
- Cross-region match rate
- Blowout rate by region

**Expected Results**:
- Low-population regions → longer search times, higher delta ping (spill to other regions)
- High-population regions → shorter search times, better ping
- Regional backoff helps but doesn't eliminate disparities

**Config Preset**: `experiments/regional_population_effects.json`

---

### Experiment 5: Skill Evolution Over Time

**Dependencies**: Slices D, C

**Parameters**: Compare "Static Skill" vs "Evolving Skill" modes over long runs (1000+ ticks)

**Metrics to Track**:
- Skill distribution evolution (mean, variance by bucket)
- Blowout rate over time
- Search time trends
- Performance distribution by skill bucket

**Expected Results**:
- Evolving skill → skill distribution may shift (e.g., players improve)
- Evolving skill → blowout rates may change as skill estimates improve
- Static skill → stable but potentially unrealistic

**Config Preset**: `experiments/skill_evolution_comparison.json`

---

### Experiment 6: Party Size Effects

**Dependencies**: Slice A

**Parameters**: Vary party size distribution (solo-only vs 50% parties of size 2-4)

**Metrics to Track**:
- Search time for solo vs party players
- Match rate by party size
- Skill disparity within parties
- Team balance quality (with parties)

**Expected Results**:
- Larger parties → longer search times (harder to find compatible matches)
- Parties maintain skill cohesion better than random groups
- Team balancing with parties is more constrained

**Config Preset**: `experiments/party_size_effects.json`

---

## Next Steps

1. **Review this roadmap** and confirm slice priorities
2. **Start with Phase 1** (Slices A + B) for core matchmaking fidelity
3. **Implement incrementally**: Complete one slice, validate, then move to next
4. **Update roadmap** as you discover gaps or adjust scope
5. **Document findings**: Add "Results" sections to slices as you complete them

---

## Notes

- **Intentional Simplifications**: Some whitepaper features are intentionally simplified or deferred:
  - Map diversity/rotation (low priority)
  - Voice chat matching (weak signal, can ignore)
  - Platform-specific optimizations (can treat as cross-platform penalty only)
- **Performance**: Current implementation handles ~5k-10k players comfortably. For larger populations, use aggregate model (Slice H).
- **Testing**: Each slice should include unit tests and integration tests. Use property-based tests where possible (e.g., party integrity, backoff monotonicity).

---

**Version**: 1.0

