# Model Variables Documentation

Complete reference for all configuration parameters in the OkQueuePD matchmaking simulation. This document describes what each variable controls, its default value, and the effects of tweaking it.

**Related Documentation:**
- [Mathematical Model](cod_matchmaking_model.md) - Full mathematical specification
- [Implementation Roadmap](COD_MM_ROADMAP.md) - Build order and experiment scenarios
- [README Configuration Section](../README.md#-configuration-parameters) - Quick reference tables

---

## Connection & Ping Parameters

These parameters control network latency constraints and how the matchmaker prioritizes connection quality.

### `maxPing`
- **Type:** `f64` (milliseconds)
- **Default:** `200.0`
- **Description:** Hard maximum acceptable ping to any data center. Players will never be matched to a data center with ping exceeding this value.
- **Effects:**
  - **Increase:** Allows matching to more distant data centers, potentially improving search times but degrading connection quality
  - **Decrease:** Restricts matching to closer data centers, improving ping quality but potentially increasing search times
- **Formula Reference:** Whitepaper §2.3, constraint `p_{i,d} ≤ P_max`
- **Use Cases:** Regional tuning (e.g., lower max ping for competitive modes), infrastructure constraints

### `deltaPingInitial`
- **Type:** `f64` (milliseconds)
- **Default:** `10.0`
- **Description:** Initial tolerance for delta ping (additional latency vs. best data center) when search starts. Part of the backoff curve that relaxes over time.
- **Effects:**
  - **Increase:** More lenient connection requirements initially, faster matches but worse ping from the start
  - **Decrease:** Stricter initial connection requirements, better ping quality but longer initial search times
- **Formula Reference:** Whitepaper §2.3, `f_conn(w) = min(δ_init + δ_rate·w, δ_max)`
- **Regional Override:** Can be set per-region via `RegionConfig`

### `deltaPingRate`
- **Type:** `f64` (milliseconds per second)
- **Default:** `2.0`
- **Description:** Rate at which delta ping tolerance increases per second of search time. Controls how quickly connection constraints relax.
- **Effects:**
  - **Increase:** Faster relaxation of ping constraints, shorter search times but more ping degradation over time
  - **Decrease:** Slower relaxation, maintains better ping quality for longer but increases search times
- **Formula Reference:** Whitepaper §2.3, backoff rate parameter
- **Regional Override:** Can be set per-region via `RegionConfig`

### `deltaPingMax`
- **Type:** `f64` (milliseconds)
- **Default:** `100.0`
- **Description:** Maximum allowed delta ping even after backoff. Caps the additional latency penalty players will accept.
- **Effects:**
  - **Increase:** Allows higher ping penalties after long waits, can reduce search times for extreme skill buckets
  - **Decrease:** Maintains better ping quality even after long waits, but may result in very long search times
- **Formula Reference:** Whitepaper §2.3, maximum backoff value

---

## Skill Similarity & Disparity Parameters

These parameters control skill-based matchmaking (SBMM) constraints: how similar players must be and how much skill spread is allowed in a lobby.

### `skillSimilarityInitial`
- **Type:** `f64` (percentile units, 0-1)
- **Default:** `0.05`
- **Description:** Initial skill similarity tolerance when search starts. Defines the allowed percentile range around a player's skill.
- **Effects:**
  - **Increase:** More lenient skill matching initially, faster matches but wider skill spreads
  - **Decrease:** Stricter skill matching, better skill parity but longer search times (especially for extreme skill buckets)
- **Formula Reference:** Whitepaper §2.7, §3.3: `[ℓ_j(t), u_j(t)] = [π̄_j - f_skill(w_j), π̄_j + f_skill(w_j)]`
- **Regional Override:** Can be set per-region via `RegionConfig`

### `skillSimilarityRate`
- **Type:** `f64` (percentile units per second)
- **Default:** `0.01`
- **Description:** Rate at which skill similarity tolerance increases per second of search time.
- **Effects:**
  - **Increase:** Faster skill constraint relaxation, shorter search times but more skill mismatches over time
  - **Decrease:** Maintains tighter skill matching for longer, better match quality but longer searches
- **Formula Reference:** Whitepaper §2.7, skill backoff rate
- **Regional Override:** Can be set per-region via `RegionConfig`

### `skillSimilarityMax`
- **Type:** `f64` (percentile units, 0-1)
- **Default:** `0.5`
- **Description:** Maximum allowed skill similarity tolerance even after long search times. Caps how wide skill ranges can become.
- **Effects:**
  - **Increase:** Allows very wide skill ranges after long waits, reduces search time ceiling but increases blowout risk
  - **Decrease:** Maintains tighter skill bounds even after long waits, better match quality but potentially very long searches for extreme skill players
- **Formula Reference:** Whitepaper §2.7, maximum skill backoff value

### `maxSkillDisparityInitial`
- **Type:** `f64` (percentile units, 0-1)
- **Default:** `0.1`
- **Description:** Initial maximum allowed skill disparity across all players in a lobby (difference between highest and lowest skill).
- **Effects:**
  - **Increase:** Allows wider skill spreads in lobbies initially, easier matchmaking but less fair matches
  - **Decrease:** Tighter skill bounds in lobbies, more balanced matches but harder to form lobbies
- **Formula Reference:** Whitepaper §2.7, §3.3: `Δπ_M ≤ Δπ^max_j(t)` for all searches j
- **Note:** Different from similarity - disparity is the range across the whole lobby, similarity is the range around each player

### `maxSkillDisparityRate`
- **Type:** `f64` (percentile units per second)
- **Default:** `0.02`
- **Description:** Rate at which maximum skill disparity increases per second of search time.
- **Effects:**
  - **Increase:** Faster relaxation of lobby skill spread constraints
  - **Decrease:** Maintains tighter lobby skill bounds for longer
- **Formula Reference:** Whitepaper §2.7, disparity backoff rate

### `maxSkillDisparityMax`
- **Type:** `f64` (percentile units, 0-1)
- **Default:** `0.8`
- **Description:** Maximum allowed skill disparity across a lobby even after long waits.
- **Effects:**
  - **Increase:** Allows very wide skill spreads (e.g., top 10% vs bottom 10%), easier matchmaking but unfair matches
  - **Decrease:** Maintains tighter skill bounds, better fairness but potentially impossible matches for extreme skill players
- **Formula Reference:** Whitepaper §2.7, maximum disparity backoff

---

## Distance Metric Weights

These parameters control the relative importance of different factors when calculating distance between search objects for candidate selection.

### `weightGeo`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.3`
- **Description:** Weight of geographic distance in the candidate distance metric. Higher values prioritize matching players who are physically closer.
- **Effects:**
  - **Increase:** More emphasis on geographic proximity, better ping quality but potentially worse skill matching
  - **Decrease:** Less emphasis on geography, can match across larger distances (higher ping) but better skill parity
- **Formula Reference:** Whitepaper §3.1: `D(j,k) = α_geo·d_geo + α_skill·d_skill + α_input·d_input + α_platform·d_platform`
- **Note:** Weights don't need to sum to 1.0 - relative ratios matter

### `weightSkill`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.4`
- **Description:** Weight of skill difference in the candidate distance metric. Higher values prioritize matching players with similar skill.
- **Effects:**
  - **Increase:** More emphasis on skill matching, better match fairness but potentially worse ping
  - **Decrease:** Less emphasis on skill, faster matches but potentially more skill mismatches
- **Formula Reference:** Whitepaper §3.1, skill component weight
- **Note:** Typically the largest weight to prioritize SBMM

### `weightInput`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.15`
- **Description:** Weight of input device mismatch penalty (controller vs. mouse+keyboard) in distance metric.
- **Effects:**
  - **Increase:** Stronger preference to match same input devices, better input parity but longer searches for mixed-input populations
  - **Decrease:** Less penalty for cross-input matching, faster matches but potential input device imbalance
- **Formula Reference:** Whitepaper §3.1, input device component
- **Use Cases:** Crossplay balancing, competitive mode input restrictions

### `weightPlatform`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.15`
- **Description:** Weight of platform mismatch penalty (PC vs. console) in distance metric.
- **Effects:**
  - **Increase:** Stronger preference for same-platform matching
  - **Decrease:** More cross-platform matching, improves matchmaking efficiency for sparse populations
- **Formula Reference:** Whitepaper §3.1, platform component
- **Use Cases:** Crossplay preferences, platform-specific server requirements

---

## Quality Score Weights

These parameters control how match quality is calculated when choosing among multiple feasible match candidates.

### `qualityWeightPing`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.4`
- **Description:** Weight of ping quality in the match quality score. Higher values prefer matches with lower average delta ping.
- **Effects:**
  - **Increase:** Prioritizes connection quality when multiple feasible matches exist, better ping but potentially worse skill balance
  - **Decrease:** Less emphasis on ping in match selection, may choose matches with higher ping if skill balance is better
- **Formula Reference:** Whitepaper §3.4: `Q(M) = β_1 Q_ping + β_2 Q_skill_balance + β_3 Q_wait_time + β_4 Q_diversity`
- **Note:** Used for tie-breaking among feasible matches, not for feasibility constraints

### `qualityWeightSkillBalance`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.4`
- **Description:** Weight of team skill balance in the match quality score. Higher values prefer more balanced team skill differences.
- **Effects:**
  - **Increase:** Prioritizes balanced team skill when multiple matches are feasible, reduces blowouts
  - **Decrease:** Less emphasis on skill balance in match selection
- **Formula Reference:** Whitepaper §3.4, skill balance component

### `qualityWeightWaitTime`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.2`
- **Description:** Weight of search time fairness in the match quality score. Higher values prioritize matching long-waiting players.
- **Effects:**
  - **Increase:** Reduces maximum search times, improves fairness but may reduce match quality for long-waiting players
  - **Decrease:** Less priority for long-waiting players, may result in some players waiting very long
- **Formula Reference:** Whitepaper §3.4, wait time fairness component

---

## Matchmaking Algorithm Parameters

These parameters control core matchmaking algorithm behavior and performance.

### `tickInterval`
- **Type:** `f64` (seconds)
- **Default:** `5.0`
- **Description:** Time between matchmaking ticks (algorithm runs every tick). Controls how frequently the matchmaker attempts to form matches.
- **Effects:**
  - **Increase:** Less frequent matchmaking attempts, lower CPU usage but slower response to new searches
  - **Decrease:** More frequent attempts, faster matchmaking response but higher CPU usage
- **Note:** All wait times and backoff curves use seconds, so this converts ticks to seconds

### `numSkillBuckets`
- **Type:** `usize` (count)
- **Default:** `10`
- **Description:** Number of skill buckets for analysis and tracking. Players are assigned to buckets based on skill percentile.
- **Effects:**
  - **Increase:** Finer-grained skill tracking, more detailed analytics but more buckets to track
  - **Decrease:** Coarser skill tracking, simpler analytics
- **Formula Reference:** Whitepaper §2.4: `b_i(t) = ⌊B·π_i(t)⌋ + 1`
- **Note:** Does not affect matchmaking algorithm directly, only analytics

### `topKCandidates`
- **Type:** `usize` (count)
- **Default:** `50`
- **Description:** Number of candidate searches to consider per seed in the greedy match construction algorithm.
- **Effects:**
  - **Increase:** More candidates evaluated, potentially better matches but slower algorithm (O(K) complexity per seed)
  - **Decrease:** Fewer candidates, faster algorithm but may miss better matches
- **Formula Reference:** Whitepaper §3.1-3.5, seed + greedy algorithm
- **Performance Note:** Main performance bottleneck - higher values slow down matchmaking significantly

### `arrivalRate`
- **Type:** `f64` (players per tick)
- **Default:** `10.0`
- **Description:** Expected number of players coming online per tick (Poisson process parameter). Controls population growth rate.
- **Effects:**
  - **Increase:** Faster population growth, more players searching, shorter search times but more server load
  - **Decrease:** Slower population growth, fewer players, potentially longer search times
- **Formula Reference:** Whitepaper §2.5, arrival process
- **Note:** Frontend auto-scales this based on population size (typically 0.2% of population per tick)

---

## Party System Parameters

### `partyPlayerFraction`
- **Type:** `f64` (fraction, 0.0-1.0)
- **Default:** `0.5`
- **Description:** Fraction of players that are automatically assigned to parties during population generation. Controls solo vs. party mix.
- **Effects:**
  - **Increase:** More players in parties, potentially longer search times for parties but better party integrity
  - **Decrease:** More solo players, faster solo matchmaking but less realistic party dynamics
- **Formula Reference:** Whitepaper §2.4, party aggregates
- **Use Cases:** Experimenting with party size effects, simulating different social gaming patterns

---

## Team Balancing & Win Probability Parameters

### `useExactTeamBalancing`
- **Type:** `bool`
- **Default:** `true`
- **Description:** Whether to use expensive exact partitioning (Karmarkar-Karp style) for small playlists (6v6) vs. faster snake draft heuristic.
- **Effects:**
  - **true:** Optimal team balance by finding best partition, better skill parity but slower (exponential complexity)
  - **false:** Faster snake draft algorithm, slightly worse balance but much faster for large parties
- **Formula Reference:** Whitepaper §3.6, team balancing with parties
- **Note:** Only affects small playlists (6v6); large playlists always use snake draft

### `gamma`
- **Type:** `f64` (dimensionless coefficient)
- **Default:** `2.0`
- **Description:** Logistic coefficient for win probability calculation. Controls how sensitive win probability is to skill differences.
- **Effects:**
  - **Increase:** More deterministic outcomes (small skill differences → larger win probability differences), more predictable but potentially less exciting matches
  - **Decrease:** More random outcomes (skill differences matter less), more upsets but less skill-based
- **Formula Reference:** Whitepaper §3.7: `P(A wins) = σ(γ·(S_A - S_B))` where σ is logistic function
- **Typical Range:** 1.0-5.0, where 2.0 is balanced

---

## Blowout Detection Parameters

These parameters control how blowouts (unbalanced matches) are detected and classified by severity.

### `blowoutSkillCoefficient`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.4`
- **Description:** Weight of team skill difference in blowout detection score. Higher values make skill imbalance more likely to trigger blowout.
- **Effects:**
  - **Increase:** More sensitive to skill differences, detects more blowouts based on skill imbalance
  - **Decrease:** Less sensitive to skill differences alone
- **Note:** Combined with `blowoutImbalanceCoefficient` to compute blowout score

### `blowoutImbalanceCoefficient`
- **Type:** `f64` (dimensionless weight)
- **Default:** `0.3`
- **Description:** Weight of win probability imbalance in blowout detection score. Higher values make one-sided win probabilities more likely to trigger blowout.
- **Effects:**
  - **Increase:** More sensitive to predicted match imbalance, detects blowouts earlier
  - **Decrease:** Requires larger imbalances to trigger blowout detection
- **Formula:** `blowout_score = blowoutSkillCoefficient * normalized_skill_diff + blowoutImbalanceCoefficient * win_prob_imbalance`

### `blowoutMildThreshold`
- **Type:** `f64` (score threshold)
- **Default:** `0.15`
- **Description:** Minimum blowout score to classify a match as a Mild blowout. Mild blowouts have minor skill imbalances.
- **Effects:**
  - **Increase:** Fewer matches classified as blowouts (even mild ones), lower blowout rate but potentially more unfair matches
  - **Decrease:** More matches classified as mild blowouts, better tracking of imbalances

### `blowoutModerateThreshold`
- **Type:** `f64` (score threshold)
- **Default:** `0.35`
- **Description:** Minimum blowout score to classify as Moderate blowout. Moderate blowouts have noticeable skill differences.
- **Effects:**
  - **Increase:** Requires larger imbalances for moderate classification, fewer moderate blowouts tracked
  - **Decrease:** More matches classified as moderate blowouts
- **Note:** Must be > `blowoutMildThreshold`

### `blowoutSevereThreshold`
- **Type:** `f64` (score threshold)
- **Default:** `0.6`
- **Description:** Minimum blowout score to classify as Severe blowout. Severe blowouts have significant skill gaps.
- **Effects:**
  - **Increase:** Only very imbalanced matches classified as severe, fewer severe blowouts
  - **Decrease:** More matches reach severe classification
- **Note:** Must be > `blowoutModerateThreshold`
- **Use Cases:** Identifying matches that are likely to cause player frustration

---

## Skill Evolution Parameters

These parameters control how player skill changes over time based on performance.

### `enableSkillEvolution`
- **Type:** `bool`
- **Default:** `true`
- **Description:** Whether skill values update based on match performance. When false, skills remain static throughout simulation.
- **Effects:**
  - **true:** Dynamic skill system, players improve/decline based on performance, more realistic but skill distribution may shift
  - **false:** Static skill system, consistent skill distribution, simpler but less realistic player progression
- **Use Cases:** Comparing static vs. evolving skill models, testing skill estimation accuracy

### `skillLearningRate`
- **Type:** `f64` (dimensionless, typically 0.001-0.1)
- **Default:** `0.01`
- **Description:** Learning rate (α) in skill update rule. Controls how much each match affects skill estimate.
- **Effects:**
  - **Increase:** Faster skill adaptation, players' skill estimates change quickly but may be noisy
  - **Decrease:** Slower adaptation, more stable skill estimates but slower to reflect true changes
- **Formula Reference:** Whitepaper §3.7: `s_i^+ = s_i^- + α·(ŷ_i - E[Y_i])`
- **Typical Range:** 0.001-0.05 for stable systems

### `performanceNoiseStd`
- **Type:** `f64` (standard deviation)
- **Default:** `0.15`
- **Description:** Standard deviation of performance noise (ε_i ~ N(0, σ²)). Controls randomness in per-match performance.
- **Effects:**
  - **Increase:** More variable performance around skill-based expectation, more upsets but noisier skill updates
  - **Decrease:** More predictable performance, cleaner skill signal but less realistic variation
- **Formula Reference:** Whitepaper §3.7, performance noise model
- **Use Cases:** Modeling different game genres (high noise = more RNG, low noise = pure skill)

### `skillUpdateBatchSize`
- **Type:** `usize` (count)
- **Default:** `10`
- **Description:** Number of matches between skill percentile recalculations. Skill percentiles are updated in batches to maintain consistency.
- **Effects:**
  - **Increase:** Less frequent percentile updates, lower CPU cost but delayed skill bucket adjustments
  - **Decrease:** More frequent updates, more accurate skill buckets but higher CPU cost
- **Note:** Raw skill updates every match; percentiles recalculated every N matches

---

## Retention Model Parameters

These parameters control the logistic retention model that predicts whether players continue playing or quit.

**Formula Reference:** Whitepaper §3.8: `P(continue) = σ(θ^T z_i)` where z_i is experience vector and θ are coefficients.

### `retentionConfig.thetaPing`
- **Type:** `f64` (coefficient)
- **Default:** `-0.02`
- **Description:** Coefficient for delta ping in retention logit. Typically negative (high ping reduces retention).
- **Effects:**
  - **More negative:** Stronger negative impact of high ping on retention, players more likely to quit with bad connection
  - **Less negative/positive:** Ping matters less for retention
- **Formula:** Logit component: `thetaPing * avg_delta_ping`

### `retentionConfig.thetaSearchTime`
- **Type:** `f64` (coefficient)
- **Default:** `-0.015`
- **Description:** Coefficient for search time in retention logit. Typically negative (long waits reduce retention).
- **Effects:**
  - **More negative:** Stronger negative impact of long search times, players quit faster if matchmaking is slow
  - **Less negative:** Search time matters less for retention
- **Formula:** Logit component: `thetaSearchTime * avg_search_time`

### `retentionConfig.thetaBlowout`
- **Type:** `f64` (coefficient)
- **Default:** `-0.5`
- **Description:** Coefficient for blowout rate in retention logit. Typically negative (blowouts reduce retention).
- **Effects:**
  - **More negative:** Stronger negative impact of blowouts, players more likely to quit after unfair matches
  - **Less negative:** Blowouts matter less for retention
- **Formula:** Logit component: `thetaBlowout * blowout_rate`
- **Note:** Largest magnitude coefficient (blowouts are biggest retention driver)

### `retentionConfig.thetaWinRate`
- **Type:** `f64` (coefficient)
- **Default:** `0.8`
- **Description:** Coefficient for win rate in retention logit. Typically positive (winning increases retention).
- **Effects:**
  - **More positive:** Stronger positive impact of winning, players more likely to continue when winning
  - **Less positive/negative:** Win rate matters less or hurts retention (rare)
- **Formula:** Logit component: `thetaWinRate * win_rate`

### `retentionConfig.thetaPerformance`
- **Type:** `f64` (coefficient)
- **Default:** `0.6`
- **Description:** Coefficient for performance index in retention logit. Typically positive (good performance increases retention).
- **Effects:**
  - **More positive:** Stronger positive impact of good performance, players continue when playing well
  - **Less positive:** Performance matters less for retention
- **Formula:** Logit component: `thetaPerformance * avg_performance`
- **Note:** Separate from win rate - captures individual performance vs. team outcome

### `retentionConfig.baseContinueProb`
- **Type:** `f64` (logit offset)
- **Default:** `0.0`
- **Description:** Base logit value before experience terms. Maps to base probability via logistic function.
- **Effects:**
  - **Increase:** Higher baseline continuation probability, more players continue playing regardless of experience
  - **Decrease:** Lower baseline, experience factors matter more
- **Formula:** Final logit = `baseContinueProb + sum(theta_i * experience_i)`, then `P = σ(logit)`
- **Note:** `0.0` means 50% baseline (logistic(0) = 0.5)

### `retentionConfig.experienceWindowSize`
- **Type:** `usize` (count)
- **Default:** `5`
- **Description:** Number of recent matches to include in experience vector for retention calculation.
- **Effects:**
  - **Increase:** Longer memory of past experiences, more stable retention predictions but slower to adapt
  - **Decrease:** Shorter memory, faster adaptation to recent changes but more volatile
- **Note:** Each match adds an experience vector; only last N are averaged for retention calculation

---

## Regional Configuration Overrides

These parameters can be set per-region to override global values. Useful for tuning matchmaking differently across geographic regions.

### `regionConfigs[Region].maxPing`
- **Type:** `Option<f64>` (milliseconds)
- **Default:** `None` (uses global `maxPing`)
- **Description:** Region-specific maximum ping override.
- **Use Cases:** Lower max ping for regions with good infrastructure, higher for regions with limited data centers

### `regionConfigs[Region].deltaPingInitial`
- **Type:** `Option<f64>` (milliseconds)
- **Default:** `None` (uses global `deltaPingInitial`)
- **Description:** Region-specific initial delta ping tolerance.
- **Use Cases:** Stricter ping requirements for competitive regions, more lenient for sparse regions

### `regionConfigs[Region].deltaPingRate`
- **Type:** `Option<f64>` (milliseconds per second)
- **Default:** `None` (uses global `deltaPingRate`)
- **Description:** Region-specific delta ping backoff rate.
- **Use Cases:** Faster relaxation for low-population regions

### `regionConfigs[Region].skillSimilarityInitial`
- **Type:** `Option<f64>` (percentile units)
- **Default:** `None` (uses global `skillSimilarityInitial`)
- **Description:** Region-specific initial skill similarity tolerance.
- **Use Cases:** Tighter SBMM for competitive regions, looser for casual regions

### `regionConfigs[Region].skillSimilarityRate`
- **Type:** `Option<f64>` (percentile units per second)
- **Default:** `None` (uses global `skillSimilarityRate`)
- **Description:** Region-specific skill similarity backoff rate.
- **Use Cases:** Faster skill relaxation for low-population regions

**Available Regions:** `NorthAmerica`, `Europe`, `AsiaPacific`, `SouthAmerica`, `Other`

**Example:** Set stricter ping requirements for AsiaPacific region:
```json
{
  "regionConfigs": {
    "AsiaPacific": {
      "maxPing": 150.0,
      "deltaPingInitial": 5.0
    }
  }
}
```

---

## Parameter Tuning Guidelines

### Quick Tuning Scenarios

**Reduce Search Times (may sacrifice quality):**
- Increase `deltaPingRate`, `skillSimilarityRate`, `maxSkillDisparityRate`
- Increase `deltaPingMax`, `skillSimilarityMax`
- Decrease `skillSimilarityInitial`, `deltaPingInitial`

**Improve Match Quality (may increase search times):**
- Decrease `deltaPingRate`, `skillSimilarityRate`
- Decrease `deltaPingMax`, `skillSimilarityMax`
- Increase `skillSimilarityInitial` (tighter initial bounds)

**Prioritize Connection Quality:**
- Increase `weightGeo`, `qualityWeightPing`
- Decrease `maxPing`, `deltaPingMax`
- Decrease `deltaPingRate` (slower relaxation)

**Prioritize Skill Matching:**
- Increase `weightSkill`, `qualityWeightSkillBalance`
- Decrease `skillSimilarityRate` (slower relaxation)
- Increase `skillSimilarityInitial` (tighter bounds)

**Reduce Blowouts:**
- Decrease `skillSimilarityRate`, `maxSkillDisparityRate`
- Increase `qualityWeightSkillBalance`
- Enable `useExactTeamBalancing`
- Increase `blowoutMildThreshold` (detect earlier)

**Increase Player Retention:**
- Increase `retentionConfig.thetaWinRate`, `retentionConfig.thetaPerformance`
- Decrease `retentionConfig.thetaBlowout` magnitude (but keep negative)
- Reduce search times (see above)
- Improve match quality to reduce blowouts

### Parameter Interactions

Many parameters interact in non-obvious ways:

- **Backoff curves** (`*Rate` parameters) work together - if one relaxes faster, others may not need to
- **Distance metric weights** are relative - only ratios matter, not absolute values
- **Quality score weights** only affect tie-breaking, not feasibility
- **Retention coefficients** should balance each other - too strong positive terms can mask problems

### Common Mistakes

1. **Setting backoff rates too high:** Causes immediate quality degradation instead of gradual relaxation
2. **Mismatched scales:** Distance metric weights should be roughly same order of magnitude
3. **Ignoring regional differences:** Low-population regions may need different tuning
4. **Over-tuning retention:** Small coefficient changes can have large effects (logistic function is sensitive)

---

## Default Configuration Summary

```rust
MatchmakingConfig {
    max_ping: 200.0,
    delta_ping_initial: 10.0,
    delta_ping_rate: 2.0,
    delta_ping_max: 100.0,
    skill_similarity_initial: 0.05,
    skill_similarity_rate: 0.01,
    skill_similarity_max: 0.5,
    max_skill_disparity_initial: 0.1,
    max_skill_disparity_rate: 0.02,
    max_skill_disparity_max: 0.8,
    weight_geo: 0.3,
    weight_skill: 0.4,
    weight_input: 0.15,
    weight_platform: 0.15,
    quality_weight_ping: 0.4,
    quality_weight_skill_balance: 0.4,
    quality_weight_wait_time: 0.2,
    party_player_fraction: 0.5,
    tick_interval: 5.0,
    num_skill_buckets: 10,
    top_k_candidates: 50,
    use_exact_team_balancing: true,
    gamma: 2.0,
    blowout_skill_coefficient: 0.4,
    blowout_imbalance_coefficient: 0.3,
    blowout_mild_threshold: 0.15,
    blowout_moderate_threshold: 0.35,
    blowout_severe_threshold: 0.6,
    skill_learning_rate: 0.01,
    performance_noise_std: 0.15,
    enable_skill_evolution: true,
    skill_update_batch_size: 10,
    region_configs: {},
    retention_config: RetentionConfig {
        theta_ping: -0.02,
        theta_search_time: -0.015,
        theta_blowout: -0.5,
        theta_win_rate: 0.8,
        theta_performance: 0.6,
        base_continue_prob: 0.0,
        experience_window_size: 5,
    },
}
```

---

## Experiment Scenarios

See [COD_MM_ROADMAP.md](COD_MM_ROADMAP.md) for detailed experiment scenarios that vary these parameters:

1. **SBMM Strictness Sweep** - Vary `skillSimilarityInitial` and `skillSimilarityRate`
2. **Ping vs Skill Tradeoff** - Vary `weightSkill` vs `weightGeo`
3. **Retention Model Comparison** - Compare different `retentionConfig` presets
4. **Regional Population Effects** - Use per-region overrides for low-pop regions
5. **Skill Evolution Over Time** - Compare static vs. evolving skill modes
6. **Party Size Effects** - Vary `partyPlayerFraction`

---

## References

- **Mathematical Model:** [cod_matchmaking_model.md](cod_matchmaking_model.md) - Full mathematical specification
- **Implementation:** [src/types.rs](../src/types.rs) - Rust type definitions and defaults
- **Frontend Config:** [web/src/MatchmakingSimulator.jsx](../web/src/MatchmakingSimulator.jsx) - JavaScript defaults
- **Experiments:** [COD_MM_ROADMAP.md](COD_MM_ROADMAP.md) - Experiment scenarios using these parameters

