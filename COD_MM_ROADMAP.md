# COD Matchmaking Whitepaper Implementation Roadmap

## Overview

This roadmap guides the implementation of a full agent-based matchmaking simulation that matches the mathematical model described in `cod_matchmaking_model.md`. The current codebase already implements a substantial portion of the whitepaper (approximately Stage 1-2), and this document breaks down the remaining work into **vertical slices** that can be implemented incrementally.

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
- ✅ Team balancing (snake draft)
- ✅ Match outcomes and blowout detection
- ✅ Basic retention/continuation logic
- ✅ Per-bucket statistics

The React frontend (`web/src/`) provides:
- ✅ Real-time visualization (charts, histograms, bucket stats)
- ✅ Parameter sweeps and experiments
- ✅ Configuration controls

---

## Current State vs Whitepaper Mapping

| Topic | Whitepaper Section | Current Implementation | Gap / To-Do |
|-------|-------------------|------------------------|-------------|
| **Player State Machine** | §2.5 | `PlayerState` enum, 4-state loop | ✅ Complete |
| **Player Attributes** | §2.2 | `Player` struct (location, platform, input, skill, playlists) | ✅ Complete |
| **DC & Ping Model** | §2.3 | `DataCenter`, `dc_pings`, `acceptable_dcs()` with backoff | ✅ Complete |
| **Skill System** | §2.4 | Raw skill, percentile, buckets | ⚠️ Static (no evolution) |
| **Search Objects** | §2.7 | `SearchObject` struct | ⚠️ Solo-only (no parties) |
| **Distance Metric** | §3.1 | `calculate_distance()` with weights | ✅ Complete |
| **Feasibility Checks** | §3.3 | `check_feasibility()` implements 6 constraints | ⚠️ Units/time conversion needs review |
| **Quality Score** | §3.4 | `calculate_quality()` with 3 components | ✅ Complete |
| **Team Balancing** | §3.6 | `balance_teams()` snake draft | ⚠️ Heuristic (not Karmarkar-Karp) |
| **Match Outcomes** | §3.7 | `determine_outcome()` logistic function | ⚠️ No performance model/KPM |
| **Skill Evolution** | §3.7 | None | ❌ Missing |
| **Retention Model** | §3.8 | Inline continuation probability | ⚠️ Ad-hoc (not formal logistic) |
| **Parties** | §2.4, §2.7 | `Party` struct exists but unused | ❌ Not integrated |
| **Region Graph** | §2.3, §6.1 | Regions as strings, no adjacency | ⚠️ Implicit (needs explicit) |
| **Under-full Lobbies** | §6.8 | Exact size match only | ⚠️ Missing |
| **Aggregate Model** | §5 | None | ❌ Optional Phase |

**Legend**: ✅ Complete | ⚠️ Partial/Needs Refinement | ❌ Missing

---

## Vertical Slices

Each vertical slice is a self-contained feature that touches engine, metrics, and optionally frontend. Slices can be implemented independently, but some have dependencies (noted below).

### Slice A: Parties & Multi-Player Search Objects

**Whitepaper References**: §2.4 (party aggregates), §2.7 (search objects), §3.6 (team balancing with parties)

**Goals**:
- Enable players to form parties and search together
- Build `SearchObject`s from parties (not just solo players)
- Maintain party integrity during matchmaking (no splitting parties across teams)
- Compute party-level skill aggregates (\(\bar{s}_P\), \(\Delta s_P\), \(\bar{\pi}_P\), \(\Delta\pi_P\))

**Engine Work**:
- **`src/types.rs`**:
  - Extend `Party` struct: add `preferred_playlists: HashSet<Playlist>`, `platforms: HashMap<Platform, usize>`, `input_devices: HashMap<InputDevice, usize>`, `avg_location: Location`
  - Add methods: `Party::from_players(players: &[&Player]) -> Party`, `Party::update_aggregates()`
- **`src/simulation.rs`**:
  - Add `parties: HashMap<usize, Party>` to `Simulation`
  - Implement `create_party(player_ids: Vec<usize>) -> usize` (random or rule-based)
  - Implement `join_party(party_id: usize, player_id: usize)`, `leave_party(party_id: usize, player_id: usize)`, `disband_party(party_id: usize)`
  - Modify `start_search()`: if player has `party_id`, create `SearchObject` from party; otherwise solo
  - Update `SearchObject` creation to compute aggregates from party members
- **`src/matchmaker.rs`**:
  - Ensure `balance_teams()` respects party boundaries (no splitting parties)
  - Update team balancing to use party-aggregated skills when assigning teams

**Frontend Work**:
- Add UI controls to create/join/leave parties (optional, can defer to later)
- Update visualization to show party sizes in search queue (optional)

**Metrics & Experiments**:
- Track: average party size, party match rate vs solo match rate, skill disparity within parties
- Experiment: Compare search times for solo vs party players

**Dependencies**: None (can be first slice)

---

### Slice B: Matchmaking Constraints & Backoff Refinement

**Whitepaper References**: §2.3 (DC backoff), §2.7 (skill backoff), §3.3 (feasibility), §6.8 (under-full lobbies)

**Goals**:
- Ensure backoff functions match whitepaper formulas exactly
- Fix units (seconds vs ticks) consistently
- Add optional under-full lobby support
- Add debug logging for feasibility failures

**Engine Work**:
- **`src/types.rs`**:
  - Review `MatchmakingConfig` backoff methods: ensure `delta_ping_backoff()`, `skill_similarity_backoff()`, `skill_disparity_backoff()` match formulas:
    - \(f_{\text{conn}}(w) = \min(\delta_{\text{init}} + \delta_{\text{rate}} \cdot w, \delta_{\text{max}})\)
    - \(f_{\text{skill}}(w) = \min(\sigma_{\text{init}} + \sigma_{\text{rate}} \cdot w, \sigma_{\text{max}})\)
  - Add config: `allow_underfull_lobbies: bool`, `underfull_threshold: f64` (e.g., 0.9 = 90% full)
  - Add config: `underfull_min_wait_seconds: f64` (only allow under-full after this wait)
- **`src/matchmaker.rs`**:
  - Fix `SearchObject::wait_time()`: ensure it returns **seconds** (multiply ticks by `tick_interval`)
  - Update all backoff calls to use seconds consistently
  - In `check_feasibility()`: add exact skill range check \([\ell_j(t), u_j(t)]\) per whitepaper §3.3
  - Add optional under-full lobby logic: if `allow_underfull_lobbies` and wait > threshold, allow matches at ≥ `underfull_threshold * required_players`
  - Add debug logging (behind feature flag `#[cfg(feature = "debug")]`) that records why feasibility checks fail
- **`src/simulation.rs`**:
  - Ensure `tick_interval` is used consistently when converting between ticks and seconds

**Frontend Work**:
- Add config sliders for under-full lobby parameters
- Show feasibility failure reasons in debug mode (optional)

**Metrics & Experiments**:
- Validate: backoff curves match expected formulas (plot curves)
- Experiment: Compare search times with/without under-full lobbies in low-population scenarios

**Dependencies**: None (can be parallel with Slice A)

---

### Slice C: Team Balancing & Blowout Modeling

**Whitepaper References**: §3.6 (team balancing), §3.7 (outcomes, blowouts)

**Goals**:
- Improve team balancing to better approximate Karmarkar-Karp partitioning
- Enhance blowout detection with more nuanced metrics
- Track blowout severity/severity buckets

**Engine Work**:
- **`src/types.rs`**:
  - Add to `Match`: `expected_score_differential: f64`, `win_probability_imbalance: f64`, `blowout_severity: Option<BlowoutSeverity>`
  - Add enum `BlowoutSeverity { Mild, Moderate, Severe }`
- **`src/matchmaker.rs`**:
  - Refactor `balance_teams()`:
    - For small playlists (6v6): implement exact or near-exact partition search (minimize team skill difference)
    - Keep snake draft as fallback for large playlists
    - Ensure parties stay intact
  - Add config: `use_exact_team_balancing: bool` (enable expensive balancing for small modes)
- **`src/simulation.rs`**:
  - Enhance `determine_outcome()`:
    - Use clearly parameterized logistic: \(P(A \text{ wins}) = \sigma(\gamma (S_A - S_B))\) with configurable \(\gamma\)
    - Compute `win_probability_imbalance` and `expected_score_differential`
    - Refactor blowout detection: use separate configurable coefficients for skill difference vs win-probability imbalance
    - Assign `blowout_severity` based on thresholds
  - Add to `SimulationStats`: `blowout_severity_counts: HashMap<BlowoutSeverity, usize>`, `per_playlist_blowout_rate: HashMap<Playlist, f64>`

**Frontend Work**:
- Add chart: blowout rate by playlist
- Add chart: blowout severity distribution
- Show team skill difference in match details (optional)

**Metrics & Experiments**:
- Track: team skill difference distribution, blowout rate by playlist, blowout severity breakdown
- Experiment: Compare blowout rates with exact vs heuristic team balancing

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

**Slices**: A (Parties) + B (Constraints/Backoff)

**Goal**: Complete the core matchmaking loop with parties and accurate constraints.

**Deliverables**:
- Parties fully integrated into search and matchmaking
- Backoff functions match whitepaper formulas exactly
- Under-full lobby support (optional)
- Debug logging for feasibility failures

**Validation**:
- Run simulation with parties and verify: party integrity maintained, search times reasonable, backoff curves match expected formulas
- Compare search times with/without under-full lobbies in low-population scenarios

**Estimated Effort**: 2-3 weeks

---

### Phase 2: Match Quality & Outcomes

**Slices**: C (Team Balancing/Blowouts) + D (Performance/Skill Evolution)

**Goal**: Improve match quality prediction and enable dynamic skill evolution.

**Deliverables**:
- Exact team balancing for small playlists
- Enhanced blowout detection with severity
- Performance model and skill update rule
- Skill distribution evolution tracking

**Validation**:
- Compare blowout rates with exact vs heuristic balancing
- Verify skill evolution: players improve/decline based on performance
- Track skill distribution stability over long runs

**Estimated Effort**: 3-4 weeks

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

### Experiment 1: SBMM Strictness Sweep

**Dependencies**: Slices A, B, C

**Parameters**: Vary `skill_similarity_initial` from 0.01 to 0.3

**Metrics to Track**:
- Search time (P50, P90, P99) by skill bucket
- Delta ping by skill bucket
- Blowout rate overall and by bucket
- Skill disparity distribution

**Expected Results**:
- Tighter SBMM → longer search times, especially for extreme skill buckets
- Tighter SBMM → lower blowout rate, better skill matching
- Tradeoff: search time vs match quality

**Config Preset**: `experiments/sbmm_strictness_sweep.json`

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

**Last Updated**: 2024-12-19  
**Version**: 1.0

