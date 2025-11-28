use crate::matchmaker::{MatchResult, Matchmaker};
use crate::types::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main simulation state and controller
#[derive(Serialize, Deserialize)]
pub struct Simulation {
    /// Current simulation time (in ticks)
    pub current_time: u64,
    /// All players in the simulation
    pub players: HashMap<usize, Player>,
    /// Data centers
    pub data_centers: Vec<DataCenter>,
    /// Active search objects
    pub searches: Vec<SearchObject>,
    /// Active matches
    pub matches: HashMap<usize, Match>,
    /// Matchmaking configuration
    pub config: MatchmakingConfig,
    /// Running statistics
    pub stats: SimulationStats,
    /// Next IDs for various entities
    next_player_id: usize,
    next_search_id: usize,
    next_match_id: usize,
    next_party_id: usize,
    /// Active parties
    pub parties: HashMap<usize, Party>,
    /// Random number generator seed
    rng_seed: u64,
    /// Arrival rate (players per tick)
    arrival_rate: f64,
    /// Number of matches since last percentile update
    matches_since_percentile_update: usize,
    /// Session tracking: total matches played across completed sessions
    total_matches_in_sessions: usize,
    /// Session tracking: continuation/quit counts by bucket (bucket -> (continues, quits))
    session_continues: HashMap<usize, (usize, usize)>,
    /// Return probability tracking: return attempts by bucket (bucket -> count)
    return_attempts_by_bucket: HashMap<usize, usize>,
    /// Return probability tracking: returns by bucket (bucket -> count)
    returns_by_bucket: HashMap<usize, usize>,
    /// Diagnostic: track computed continue probabilities (for verification)
    continue_prob_samples: Vec<f64>,
    /// Diagnostic: track logit values and experience vectors (for debugging)
    logit_samples: Vec<f64>,
    experience_samples: Vec<(f64, f64, f64, f64, f64)>, // (delta_ping, search_time, blowout_rate, win_rate, performance)
}

impl Simulation {
    pub fn new(config: MatchmakingConfig, seed: u64) -> Self {
        Self {
            current_time: 0,
            players: HashMap::new(),
            data_centers: Vec::new(),
            searches: Vec::new(),
            matches: HashMap::new(),
            config,
            stats: {
                let mut stats = SimulationStats::default();
                stats.churn_threshold_ticks = 100; // Default: 100 ticks
                stats
            },
            next_player_id: 0,
            next_search_id: 0,
            next_match_id: 0,
            next_party_id: 0,
            parties: HashMap::new(),
            rng_seed: seed,
            arrival_rate: 10.0,
            matches_since_percentile_update: 0,
            total_matches_in_sessions: 0,
            session_continues: HashMap::new(),
            return_attempts_by_bucket: HashMap::new(),
            returns_by_bucket: HashMap::new(),
            continue_prob_samples: Vec::new(),
            logit_samples: Vec::new(),
            experience_samples: Vec::new(),
        }
    }

    /// Initialize with default data centers (global distribution)
    pub fn init_default_data_centers(&mut self) {
        let dcs = vec![
            ("US-East", Location::new(39.0, -77.0), "NA"),
            ("US-West", Location::new(37.0, -122.0), "NA"),
            ("US-Central", Location::new(41.0, -96.0), "NA"),
            ("EU-West", Location::new(51.0, 0.0), "EU"),
            ("EU-Central", Location::new(50.0, 8.0), "EU"),
            ("EU-North", Location::new(59.0, 18.0), "EU"),
            ("Asia-East", Location::new(35.0, 139.0), "APAC"),
            ("Asia-SE", Location::new(1.0, 103.0), "APAC"),
            ("Australia", Location::new(-33.0, 151.0), "APAC"),
            ("South-America", Location::new(-23.0, -46.0), "SA"),
        ];

        for (i, (name, location, region)) in dcs.into_iter().enumerate() {
            self.data_centers.push(DataCenter::new(i, name, location, region));
        }
    }

    /// Generate a population of players
    pub fn generate_population(&mut self, count: usize, region_weights: Option<Vec<(Location, f64)>>) {
        let mut rng = StdRng::seed_from_u64(self.rng_seed);

        let regions = region_weights.unwrap_or_else(|| vec![
            (Location::new(39.0, -95.0), 0.35),   // NA
            (Location::new(50.0, 10.0), 0.30),    // EU
            (Location::new(35.0, 105.0), 0.20),   // Asia
            (Location::new(-25.0, 135.0), 0.08), // Australia
            (Location::new(-15.0, -55.0), 0.07), // SA
        ]);

        for _ in 0..count {
            // Select region based on weights
            let r: f64 = rng.gen();
            let mut cumulative = 0.0;
            let mut region_loc = regions[0].0;
            for (loc, weight) in &regions {
                cumulative += weight;
                if r < cumulative {
                    region_loc = *loc;
                    break;
                }
            }

            // Add some randomness to location within region
            let location = Location::new(
                region_loc.lat + rng.gen_range(-10.0..10.0),
                region_loc.lon + rng.gen_range(-15.0..15.0),
            );

            // Generate skill using a normal-ish distribution
            let skill = self.generate_skill(&mut rng);

            let mut player = Player::new(self.next_player_id, location, skill);
            self.next_player_id += 1;

            // Randomize platform and input
            player.platform = match rng.gen_range(0..3) {
                0 => Platform::PC,
                1 => Platform::PlayStation,
                _ => Platform::Xbox,
            };

            player.input_device = if player.platform == Platform::PC {
                if rng.gen_bool(0.7) {
                    InputDevice::MouseKeyboard
                } else {
                    InputDevice::Controller
                }
            } else {
                if rng.gen_bool(0.9) {
                    InputDevice::Controller
                } else {
                    InputDevice::MouseKeyboard
                }
            };

            // Calculate pings to all DCs
            for dc in &self.data_centers {
                let base_distance = location.distance_km(&dc.location);
                // Ping model: ~1ms per 100km + base latency + jitter
                let base_ping = base_distance / 100.0 + 15.0;
                let jitter = rng.gen_range(-5.0..10.0);
                let ping = (base_ping + jitter).max(10.0);
                player.dc_pings.insert(dc.id, ping);
            }

            // Find best DC
            if let Some((&best_dc, &best_ping)) = player.dc_pings.iter()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            {
                player.best_dc = Some(best_dc);
                player.best_ping = best_ping;
            }

            // Set preferred playlists
            player.preferred_playlists.clear();
            player.preferred_playlists.insert(Playlist::TeamDeathmatch);
            if rng.gen_bool(0.4) {
                player.preferred_playlists.insert(Playlist::Domination);
            }
            if rng.gen_bool(0.2) {
                player.preferred_playlists.insert(Playlist::SearchAndDestroy);
            }
            if rng.gen_bool(0.15) {
                player.preferred_playlists.insert(Playlist::GroundWar);
            }
            if rng.gen_bool(0.1) {
                player.preferred_playlists.insert(Playlist::FreeForAll);
            }

            // Start offline
            player.state = PlayerState::Offline;

            self.players.insert(player.id, player);
        }

        // Calculate skill percentiles
        self.update_skill_percentiles();

        // ---------------------------------------------------------------------
        // Auto-generate parties from the population based on config
        // ---------------------------------------------------------------------
        let target_fraction = self
            .config
            .party_player_fraction
            .clamp(0.0, 1.0);

        if target_fraction > 0.0 && self.players.len() >= 2 {
            use rand::seq::SliceRandom;

            // Re-seed RNG so party generation is stable given the same seed
            let mut rng = StdRng::seed_from_u64(self.rng_seed.wrapping_add(1));

            let mut player_ids: Vec<usize> = self.players.keys().copied().collect();
            player_ids.shuffle(&mut rng);

            let total_players = player_ids.len();
            let target_party_players =
                ((total_players as f64) * target_fraction).round() as usize;

            let mut assigned_players = 0usize;
            let mut idx = 0usize;

            while idx + 1 < total_players && assigned_players < target_party_players {
                let remaining = total_players - idx;

                // Sample a party size between 2-4, capped by remaining players
                let max_size = remaining.min(4);
                if max_size < 2 {
                    break;
                }

                let size = match max_size {
                    2 => 2,
                    3 => if rng.gen_bool(0.6) { 3 } else { 2 },
                    _ => {
                        // For 4+ remaining players, bias slightly toward 2-3 person parties
                        let r: f64 = rng.gen();
                        if r < 0.5 {
                            2
                        } else if r < 0.85 {
                            3
                        } else {
                            4
                        }
                    }
                };

                if idx + size > total_players {
                    break;
                }

                let party_slice = &player_ids[idx..idx + size];
                let party_member_ids: Vec<usize> = party_slice.to_vec();

                // Ignore errors (e.g., if any player is already in a party)
                if self.create_party(party_member_ids).is_ok() {
                    assigned_players += size;
                }

                idx += size;
            }
        }
    }

    /// Generate skill value using a beta-like distribution
    fn generate_skill(&self, rng: &mut impl Rng) -> f64 {
        // Use sum of uniform randoms to approximate normal distribution
        let sum: f64 = (0..12).map(|_| rng.gen::<f64>()).sum();
        let normalized = (sum - 6.0) / 3.0; // Roughly N(0,1)
        normalized.clamp(-1.0, 1.0)
    }

    /// Generate performance index for a player in a match
    /// Per whitepaper §3.7: Y_i = f_perf(s_i, s_lobby, m) + ε_i
    fn generate_performance(
        &self,
        player: &Player,
        lobby_avg_skill: f64,
        _playlist: Playlist,
        rng: &mut impl Rng,
    ) -> f64 {
        // Base performance: f_perf(s_i, s_lobby, m)
        // Higher skill → higher base performance
        // Performance relative to lobby average
        let skill_advantage = player.skill - lobby_avg_skill;
        
        // Base performance increases with skill and advantage
        // Normalize to 0-1 scale: 0.3 base + skill contribution + advantage
        let base_perf = 0.3 + (player.skill + 1.0) / 2.0 * 0.4 + skill_advantage * 0.2;
        
        // Add noise: ε_i ~ N(0, σ²)
        // Using uniform approximation for simplicity (range = ±3σ covers ~99.7%)
        let noise_range = self.config.performance_noise_std * 3.0;
        let noise = rng.gen_range(-noise_range..noise_range);
        
        // Clamp to [0, 1]
        (base_perf + noise).clamp(0.0, 1.0)
    }

    /// Compute expected performance (deterministic part, no noise)
    /// E[Y_i | s_i, lobby] = f_perf(s_i, s_lobby, m)
    fn compute_expected_performance(
        &self,
        player: &Player,
        lobby_avg_skill: f64,
    ) -> f64 {
        // E[Y_i | s_i, lobby] = deterministic part (no noise)
        let skill_advantage = player.skill - lobby_avg_skill;
        let base_perf = 0.3 + (player.skill + 1.0) / 2.0 * 0.4 + skill_advantage * 0.2;
        base_perf.clamp(0.0, 1.0)
    }

    /// Record a snapshot of skill distribution over time
    fn record_skill_distribution_snapshot(&mut self) {
        if !self.config.enable_skill_evolution {
            return;
        }
        
        // Compute mean skill per bucket
        let mut bucket_skills: HashMap<usize, Vec<f64>> = HashMap::new();
        
        for player in self.players.values() {
            bucket_skills
                .entry(player.skill_bucket)
                .or_insert_with(Vec::new)
                .push(player.skill);
        }
        
        let snapshot: Vec<(usize, f64)> = bucket_skills
            .iter()
            .map(|(&bucket_id, skills)| {
                let mean = skills.iter().sum::<f64>() / skills.len() as f64;
                (bucket_id, mean)
            })
            .collect();
        
        self.stats.skill_distribution_over_time.push((self.current_time, snapshot));
        
        // Limit history to last 1000 snapshots to prevent unbounded growth
        if self.stats.skill_distribution_over_time.len() > 1000 {
            self.stats.skill_distribution_over_time.remove(0);
        }
    }

    /// Update skill percentiles for all players
    pub fn update_skill_percentiles(&mut self) {
        let mut skills: Vec<(usize, f64)> = self.players
            .iter()
            .map(|(&id, p)| (id, p.skill))
            .collect();
        
        skills.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        
        let n = skills.len() as f64;
        for (rank, (id, _)) in skills.into_iter().enumerate() {
            if let Some(player) = self.players.get_mut(&id) {
                player.skill_percentile = (rank as f64 + 0.5) / n;
                player.update_skill_bucket(self.config.num_skill_buckets);
            }
        }
    }

    /// Bring players online based on arrival rate and return probability
    pub fn process_arrivals(&mut self, rng: &mut impl Rng) {
        // Collect offline players with their return probabilities
        let mut candidates: Vec<(usize, usize, f64)> = Vec::new(); // (player_id, bucket, return_prob)
        
        for (&player_id, player) in &self.players {
            if player.state == PlayerState::Offline {
                let return_prob = self.compute_return_probability(player);
                let bucket = player.skill_bucket;
                candidates.push((player_id, bucket, return_prob));
                
                // Track return attempt by bucket
                *self.return_attempts_by_bucket.entry(bucket).or_insert(0) += 1;
                self.stats.total_return_attempts += 1;
            }
        }

        // If no candidates, nothing to process
        if candidates.is_empty() {
            return;
        }

        // Sample arrivals using return probability (threshold-based)
        // Each candidate has a chance to return based on their return probability
        let mut arrivals: Vec<usize> = Vec::new();
        for (player_id, bucket, return_prob) in candidates {
            if rng.gen_bool(return_prob) {
                arrivals.push(player_id);
                
                // Track return by bucket
                *self.returns_by_bucket.entry(bucket).or_insert(0) += 1;
                self.stats.total_returns += 1;
            }
        }

        // Limit arrivals to Poisson rate (if we have too many, randomly sample)
        let num_arrivals = self.poisson_sample(self.arrival_rate, rng);
        if arrivals.len() > num_arrivals {
            // Shuffle and take first N
            use rand::seq::SliceRandom;
            arrivals.as_mut_slice().shuffle(rng);
            arrivals.truncate(num_arrivals);
        }

        for player_id in arrivals {
            if let Some(player) = self.players.get_mut(&player_id) {
                // Start new session when player comes online
                if player.state == PlayerState::Offline {
                    player.session_start_time = Some(self.current_time);
                    player.matches_in_session = 0;
                    // Don't clear last_session_experience - we need it for return probability
                    // Only clear last_session_end_time since they're now active again
                    // (but keep experience for potential future return calculations)
                    player.last_session_end_time = None;
                }
                player.state = PlayerState::InLobby;
            }
        }
    }

    /// Move lobby players to searching
    pub fn process_search_starts(&mut self, rng: &mut impl Rng) {
        let lobby_players: Vec<usize> = self.players
            .iter()
            .filter(|(_, p)| p.state == PlayerState::InLobby)
            .map(|(&id, _)| id)
            .collect();

        // Each lobby player has a chance to start searching
        for player_id in lobby_players {
            if rng.gen_bool(0.3) {
                self.start_search(player_id);
            }
        }
    }

    /// Start a search for a player
    fn start_search(&mut self, player_id: usize) {
        let player = match self.players.get(&player_id) {
            Some(p) => p,
            None => return,
        };

        // Check if player is in a party
        if let Some(party_id) = player.party_id {
            // Party search: only leader can start search
            if let Some(party) = self.parties.get(&party_id) {
                if party.leader_id != player_id {
                    // Not the leader, cannot start search
                    return;
                }

                // Check if all party members are ready (IN_LOBBY state)
                let all_ready = party.player_ids.iter().all(|&pid| {
                    self.players.get(&pid)
                        .map(|p| p.state == PlayerState::InLobby)
                        .unwrap_or(false)
                });

                if !all_ready {
                    // Not all members are ready
                    return;
                }

                // Set all party members to SEARCHING
                for &pid in &party.player_ids {
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.state = PlayerState::Searching;
                        p.search_start_time = Some(self.current_time);
                    }
                }

                // Create SearchObject from party
                let search = party.to_search_object(
                    self.next_search_id,
                    self.current_time,
                    &self.players,
                    &self.config,
                );

                self.next_search_id += 1;
                self.searches.push(search);
                return;
            }
        }

        // Solo search (existing logic)
        let player = match self.players.get_mut(&player_id) {
            Some(p) => p,
            None => return,
        };

        player.state = PlayerState::Searching;
        player.search_start_time = Some(self.current_time);

        // Create search object
        let search = SearchObject {
            id: self.next_search_id,
            player_ids: vec![player_id],
            avg_skill_percentile: player.skill_percentile,
            skill_disparity: 0.0,
            avg_location: player.location,
            platforms: {
                let mut m = HashMap::new();
                m.insert(player.platform, 1);
                m
            },
            input_devices: {
                let mut m = HashMap::new();
                m.insert(player.input_device, 1);
                m
            },
            acceptable_playlists: player.preferred_playlists.clone(),
            search_start_time: self.current_time,
            acceptable_dcs: player.dc_pings.keys().copied().collect(),
        };

        self.next_search_id += 1;
        self.searches.push(search);
    }

    /// Run matchmaking tick
    pub fn run_matchmaking(&mut self) -> Vec<MatchResult> {
        let mut rng = StdRng::seed_from_u64(self.rng_seed.wrapping_add(self.current_time));
        let matchmaker = Matchmaker::new(self.config.clone());

        matchmaker.run_tick(
            &mut self.searches,
            &mut self.players,
            &mut self.data_centers,
            &self.parties,
            self.current_time,
            &mut rng,
        )
    }

    /// Process match results and create matches
    pub fn create_matches(&mut self, results: Vec<MatchResult>, rng: &mut impl Rng) {
        for result in results {
            let match_id = self.next_match_id;
            self.next_match_id += 1;

            // Calculate team skills
            let team_skills: Vec<f64> = result.teams
                .iter()
                .map(|team| {
                    team.iter()
                        .filter_map(|&id| self.players.get(&id))
                        .map(|p| p.skill)
                        .sum::<f64>() / team.len() as f64
                })
                .collect();

            // Calculate team skill difference and win probability
            let team_skill_diff = if team_skills.len() >= 2 {
                team_skills[0] - team_skills[1]
            } else {
                0.0
            };
            
            // Track team skill difference for statistics
            self.stats.team_skill_difference_samples.push(team_skill_diff.abs());

            // Calculate win probability using configurable logistic
            let win_prob_team0 = if team_skills.len() >= 2 {
                let gamma = self.config.gamma;
                let logistic = 1.0 / (1.0 + (-gamma * team_skill_diff).exp());
                logistic.clamp(0.0, 1.0)
            } else {
                0.5
            };
            
            // Ensure probability is valid
            let win_prob_team0 = if win_prob_team0.is_finite() { win_prob_team0 } else { 0.5 };
            
            // Calculate win probability imbalance (0-1 scale)
            let win_probability_imbalance = (win_prob_team0 - 0.5).abs() * 2.0;
            
            // Calculate expected score differential
            // For team-based modes, scale by typical score range (e.g., 0-100 for TDM)
            let expected_score_differential = match result.playlist {
                Playlist::TeamDeathmatch => team_skill_diff * 30.0, // Rough scaling
                Playlist::SearchAndDestroy => team_skill_diff * 6.0,  // Rounds won
                Playlist::Domination => team_skill_diff * 200.0,     // Points
                Playlist::GroundWar => team_skill_diff * 100.0,      // Points
                Playlist::FreeForAll => team_skill_diff * 20.0,      // Kills
            };

            // Calculate match duration with some variance
            let base_duration = result.playlist.avg_match_duration_seconds();
            let duration_variance = rng.gen_range(0.8..1.2);
            let duration_ticks = ((base_duration * duration_variance) / self.config.tick_interval) as u64;

            let game_match = Match {
                id: match_id,
                playlist: result.playlist,
                data_center_id: result.data_center_id,
                teams: result.teams.clone(),
                start_time: self.current_time,
                expected_duration: duration_ticks,
                team_skills,
                quality_score: result.quality_score,
                skill_disparity: result.skill_disparity,
                avg_delta_ping: result.avg_delta_ping,
                expected_score_differential,
                win_probability_imbalance,
                blowout_severity: None, // Will be assigned in determine_outcome()
                player_performances: HashMap::new(),
            };

            // Check if match involves parties
            let has_party = result.player_ids.iter().any(|&pid| {
                self.players.get(&pid)
                    .map(|p| p.party_id.is_some())
                    .unwrap_or(false)
            });

            if has_party {
                self.stats.party_match_count += 1;
            } else {
                self.stats.solo_match_count += 1;
            }

            // Update player states
            for &player_id in &result.player_ids {
                if let Some(player) = self.players.get_mut(&player_id) {
                    // Record search time
                    if let Some(start) = player.search_start_time {
                        let search_time = (self.current_time - start) as f64 * self.config.tick_interval;
                        player.recent_search_times.push(search_time);
                        if player.recent_search_times.len() > 10 {
                            player.recent_search_times.remove(0);
                        }
                        self.stats.search_time_samples.push(search_time);
                        
                        // Track party vs solo search times
                        if player.party_id.is_some() {
                            self.stats.party_search_times.push(search_time);
                        } else {
                            self.stats.solo_search_times.push(search_time);
                        }
                    }

                    // Record delta ping
                    if let Some(&ping) = player.dc_pings.get(&result.data_center_id) {
                        let delta_ping = ping - player.best_ping;
                        player.recent_delta_pings.push(delta_ping);
                        if player.recent_delta_pings.len() > 10 {
                            player.recent_delta_pings.remove(0);
                        }
                        self.stats.delta_ping_samples.push(delta_ping);
                    }

                    player.state = PlayerState::InMatch;
                    player.current_match = Some(match_id);
                    player.search_start_time = None;
                }
            }

            // Record skill disparity
            self.stats.skill_disparity_samples.push(result.skill_disparity);

            self.matches.insert(match_id, game_match);
            self.stats.total_matches += 1;
        }
    }

    /// Process match completions
    pub fn process_match_completions(&mut self, rng: &mut impl Rng) {
        let completed_matches: Vec<usize> = self.matches
            .iter()
            .filter(|(_, m)| self.current_time >= m.start_time + m.expected_duration)
            .map(|(&id, _)| id)
            .collect();

        for match_id in completed_matches {
            if let Some(mut game_match) = self.matches.remove(&match_id) {
                // Release server
                if let Some(dc) = self.data_centers.iter_mut().find(|dc| dc.id == game_match.data_center_id) {
                    if let Some(busy) = dc.busy_servers.get_mut(&game_match.playlist) {
                        *busy = busy.saturating_sub(1);
                    }
                }

                // Track per-playlist match count
                *self.stats.per_playlist_match_counts.entry(game_match.playlist).or_insert(0) += 1;

                // Determine match outcome
                let (winning_team, is_blowout, blowout_severity) = self.determine_outcome(&mut game_match, rng);
                
                if is_blowout {
                    self.stats.blowout_count += 1;
                    // Track per-playlist blowout count
                    *self.stats.per_playlist_blowout_counts.entry(game_match.playlist).or_insert(0) += 1;
                }
                
                // Track blowout severity
                if let Some(severity) = blowout_severity {
                    *self.stats.blowout_severity_counts.entry(severity).or_insert(0) += 1;
                }

                // Compute performance indices and update skills
                // 1. Compute lobby average skill
                let all_player_ids: Vec<usize> = game_match.teams.iter().flatten().copied().collect();
                let lobby_avg_skill = if !all_player_ids.is_empty() {
                    all_player_ids.iter()
                        .filter_map(|&pid| self.players.get(&pid).map(|p| p.skill))
                        .sum::<f64>() / all_player_ids.len() as f64
                } else {
                    0.0
                };

                // 2. Generate performance for each player and update skills
                for &player_id in &all_player_ids {
                    // Get player immutably first to compute expected performance
                    let expected_perf = if let Some(player) = self.players.get(&player_id) {
                        if self.config.enable_skill_evolution {
                            self.compute_expected_performance(player, lobby_avg_skill)
                        } else {
                            0.0
                        }
                    } else {
                        continue;
                    };
                    
                    // Generate performance index (need player reference again)
                    let performance = if let Some(player) = self.players.get(&player_id) {
                        self.generate_performance(
                            player,
                            lobby_avg_skill,
                            game_match.playlist,
                            rng,
                        )
                    } else {
                        continue;
                    };
                    
                    // Store in match
                    game_match.player_performances.insert(player_id, performance);
                    
                    // Track performance sample
                    self.stats.performance_samples.push(performance);
                    if self.stats.performance_samples.len() > 1000 {
                        self.stats.performance_samples.remove(0);
                    }
                    
                    // Update skill if evolution is enabled
                    if self.config.enable_skill_evolution {
                        if let Some(player_mut) = self.players.get_mut(&player_id) {
                            // Normalize observed performance (simple: use raw performance)
                            let observed_perf = performance;
                            
                            // Skill update: s_i^+ = s_i^- + α(ŷ_i - E[Y_i])
                            let skill_update = self.config.skill_learning_rate * (observed_perf - expected_perf);
                            player_mut.skill = (player_mut.skill + skill_update).clamp(-1.0, 1.0);
                            
                            // Track performance in rolling window
                            player_mut.recent_performance.push(performance);
                            if player_mut.recent_performance.len() > 10 {
                                player_mut.recent_performance.remove(0);
                            }
                            
                            self.stats.total_skill_updates += 1;
                        }
                    }
                }

                // 3. Batch update percentiles if needed
                if self.config.enable_skill_evolution {
                    self.matches_since_percentile_update += 1;
                    if self.matches_since_percentile_update >= self.config.skill_update_batch_size {
                        self.update_skill_percentiles();
                        self.record_skill_distribution_snapshot();
                        self.matches_since_percentile_update = 0;
                    }
                }

                // Update player stats and decide if they continue
                for (team_idx, team) in game_match.teams.iter().enumerate() {
                    let won = team_idx == winning_team;
                    
                    for &player_id in team {
                        // Get immutable reference first to compute continue probability
                        let (match_delta_ping, match_search_time, match_performance, bucket, matches_in_session) = {
                            if let Some(player) = self.players.get(&player_id) {
                                (
                                    player.recent_delta_pings.last().copied().unwrap_or(0.0),
                                    player.recent_search_times.last().copied().unwrap_or(0.0),
                                    game_match.player_performances.get(&player_id).copied().unwrap_or(0.5),
                                    player.skill_bucket,
                                    player.matches_in_session,
                                )
                            } else {
                                continue;
                            }
                        };
                        
                        // Build experience vector for this match
                        let experience = ExperienceVector {
                            avg_delta_ping: match_delta_ping,
                            avg_search_time: match_search_time,
                            was_blowout: is_blowout,
                            won,
                            performance: match_performance,
                        };
                        
                        // Now get mutable reference to update player
                        if let Some(player) = self.players.get_mut(&player_id) {
                            player.matches_played += 1;
                            if won {
                                player.wins += 1;
                            } else {
                                player.losses += 1;
                            }

                            player.recent_blowouts.push(is_blowout);
                            if player.recent_blowouts.len() > 10 {
                                player.recent_blowouts.remove(0);
                            }

                            player.current_match = None;
                            
                            // Add to recent experience (maintain window size)
                            player.recent_experience.push(experience);
                            let window_size = self.config.retention_config.experience_window_size;
                            if player.recent_experience.len() > window_size {
                                player.recent_experience.remove(0);
                            }
                            
                            // Calculate continue probability using formal logistic model
                            // Clone necessary data to avoid borrow checker issues
                            let recent_experience_clone = player.recent_experience.clone();
                            let continue_prob = {
                                let config = &self.config.retention_config;
                                if recent_experience_clone.is_empty() {
                                    let logit = config.base_continue_prob;
                                    let prob = 1.0 / (1.0 + (-logit).exp());
                                    prob.clamp(0.0, 1.0)
                                } else {
                                    let window_size = config.experience_window_size.min(recent_experience_clone.len());
                                    let recent = &recent_experience_clone[recent_experience_clone.len().saturating_sub(window_size)..];
                                    
                                    let avg_delta_ping = recent.iter().map(|e| e.avg_delta_ping).sum::<f64>() / recent.len() as f64;
                                    let avg_search_time = recent.iter().map(|e| e.avg_search_time).sum::<f64>() / recent.len() as f64;
                                    let blowout_rate = recent.iter().filter(|e| e.was_blowout).count() as f64 / recent.len() as f64;
                                    let win_rate = recent.iter().filter(|e| e.won).count() as f64 / recent.len() as f64;
                                    let avg_performance = recent.iter().map(|e| e.performance).sum::<f64>() / recent.len() as f64;
                                    
                                    // Debug: Log first few calculations to verify math
                                    let ping_term = config.theta_ping * avg_delta_ping;
                                    let search_term = config.theta_search_time * avg_search_time;
                                    let blowout_term = config.theta_blowout * blowout_rate;
                                    let win_term = config.theta_win_rate * win_rate;
                                    let perf_term = config.theta_performance * avg_performance;
                                    
                                    let logit = config.base_continue_prob
                                        + ping_term
                                        + search_term
                                        + blowout_term
                                        + win_term
                                        + perf_term;
                                    
                                    // Track diagnostic samples (keep last 100)
                                    self.logit_samples.push(logit);
                                    if self.logit_samples.len() > 100 {
                                        self.logit_samples.remove(0);
                                    }
                                    self.experience_samples.push((avg_delta_ping, avg_search_time, blowout_rate, win_rate, avg_performance));
                                    if self.experience_samples.len() > 100 {
                                        self.experience_samples.remove(0);
                                    }
                                    
                                    let prob = 1.0 / (1.0 + (-logit).exp());
                                    let prob = prob.clamp(0.0, 1.0);
                                    if prob.is_finite() { prob } else { 0.5 }
                                }
                            };
                            
                            // Track continuation decision for statistics
                            let (continues, quits) = self.session_continues.entry(bucket).or_insert((0, 0));
                            
                            // Track computed probability for diagnostics (keep last 1000 samples)
                            self.continue_prob_samples.push(continue_prob);
                            if self.continue_prob_samples.len() > 1000 {
                                self.continue_prob_samples.remove(0);
                            }
                            
                            if rng.gen_bool(continue_prob) {
                                // Player continues
                                *continues += 1;
                                player.state = PlayerState::InLobby;
                                player.matches_in_session += 1;
                            } else {
                                // Player quits
                                *quits += 1;
                                
                                // Track quit for leaving rate calculation
                                self.stats.recent_quits.push((self.current_time, 1));
                                
                                // Preserve session experience for return probability calculation
                                player.last_session_experience = player.recent_experience.clone();
                                player.last_session_end_time = Some(self.current_time);
                                
                                // Clear current session experience (will be rebuilt in next session)
                                player.recent_experience.clear();
                                
                                player.state = PlayerState::Offline;
                                
                                // Record session completion
                                if matches_in_session > 0 {
                                    let session_length = matches_in_session;
                                    self.total_matches_in_sessions += session_length;
                                    self.stats.total_sessions_completed += 1;
                                    
                                    // Record in distribution (extend if needed)
                                    while self.stats.session_length_distribution.len() <= session_length {
                                        self.stats.session_length_distribution.push(0);
                                    }
                                    self.stats.session_length_distribution[session_length] += 1;
                                    
                                    // Clear session tracking
                                    player.session_start_time = None;
                                    player.matches_in_session = 0;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Determine match outcome using skill difference
    /// Returns (winning_team, is_blowout, blowout_severity)
    fn determine_outcome(&self, game_match: &mut Match, rng: &mut impl Rng) -> (usize, bool, Option<BlowoutSeverity>) {
        if game_match.team_skills.len() < 2 {
            return (0, false, None);
        }

        let skill_diff = game_match.team_skills[0] - game_match.team_skills[1];
        let skill_diff_abs = skill_diff.abs();
        
        // Use configurable gamma for win probability calculation
        let gamma = self.config.gamma;
        let p_team0_wins = (1.0 / (1.0 + (-gamma * skill_diff).exp())).clamp(0.0, 1.0);
        
        // Ensure probability is valid (not NaN or infinite)
        let p_team0_wins = if p_team0_wins.is_finite() { p_team0_wins } else { 0.5 };
        
        let winning_team = if rng.gen_bool(p_team0_wins) { 0 } else { 1 };
        
        // Use win_probability_imbalance already calculated in match (or recalculate if needed)
        let win_prob_imbalance = game_match.win_probability_imbalance;
        
        // Normalize skill difference to 0-1 scale (assuming max skill diff of ~2.0)
        let normalized_skill_diff = (skill_diff_abs / 2.0).min(1.0);
        
        // Blowout probability using configurable coefficients
        let blowout_prob = self.config.blowout_skill_coefficient * normalized_skill_diff
            + self.config.blowout_imbalance_coefficient * win_prob_imbalance;
        
        // Ensure blowout probability is valid
        let blowout_prob = blowout_prob.clamp(0.0, 1.0);
        let blowout_prob = if blowout_prob.is_finite() { blowout_prob } else { 0.0 };
        
        let is_blowout = rng.gen_bool(blowout_prob);
        
        // Assign blowout severity based on thresholds
        let blowout_severity = if !is_blowout {
            None
        } else if blowout_prob < self.config.blowout_mild_threshold {
            None // Too mild to classify
        } else if blowout_prob < self.config.blowout_moderate_threshold {
            Some(BlowoutSeverity::Mild)
        } else if blowout_prob < self.config.blowout_severe_threshold {
            Some(BlowoutSeverity::Moderate)
        } else {
            Some(BlowoutSeverity::Severe)
        };
        
        // Store severity in match
        game_match.blowout_severity = blowout_severity;
        
        (winning_team, is_blowout, blowout_severity)
    }

    /// Calculate probability of player continuing based on experience
    /// Per whitepaper §3.8: P(continue) = σ(θ^T z_i)
    /// where z_i is the experience vector and θ are the retention coefficients
    #[allow(dead_code)] // Keep old method name for now, will remove later
    fn calculate_continue_probability(&self, player: &Player) -> f64 {
        self.compute_continue_probability(player)
    }

    /// Calculate probability of player continuing based on experience
    /// Compute return probability based on last session's experience
    /// Per whitepaper §3.8: P(return) = σ(θ^T z_i)
    /// where z_i is the experience vector from last session and θ are the retention coefficients
    fn compute_return_probability(&self, player: &Player) -> f64 {
        let config = &self.config.retention_config;
        
        // If no last session experience, use base probability
        // Try last_session_experience first, fall back to recent_experience if available
        let experience_source = if !player.last_session_experience.is_empty() {
            &player.last_session_experience
        } else if !player.recent_experience.is_empty() {
            // Fallback: use current session experience if last session not preserved yet
            &player.recent_experience
        } else {
            // New players or players with no experience: use base probability
            let logit = config.base_continue_prob;
            let prob = 1.0 / (1.0 + (-logit).exp());
            return prob.clamp(0.0, 1.0);
        };
        
        // Build experience vector from last session (last N matches)
        let window_size = config.experience_window_size.min(experience_source.len());
        let recent = &experience_source[experience_source.len().saturating_sub(window_size)..];
        
        // Compute averages across recent matches
        let avg_delta_ping = recent.iter()
            .map(|e| e.avg_delta_ping)
            .sum::<f64>() / recent.len() as f64;
        
        let avg_search_time = recent.iter()
            .map(|e| e.avg_search_time)
            .sum::<f64>() / recent.len() as f64;
        
        let blowout_rate = recent.iter()
            .filter(|e| e.was_blowout)
            .count() as f64 / recent.len() as f64;
        
        let win_rate = recent.iter()
            .filter(|e| e.won)
            .count() as f64 / recent.len() as f64;
        
        let avg_performance = recent.iter()
            .map(|e| e.performance)
            .sum::<f64>() / recent.len() as f64;
        
        // Apply logistic model: P(return) = σ(base + θ^T z)
        // z = [avg_delta_ping, avg_search_time, blowout_rate, win_rate, avg_performance]
        // θ = [theta_ping, theta_search_time, theta_blowout, theta_win_rate, theta_performance]
        let logit = config.base_continue_prob
            + config.theta_ping * avg_delta_ping
            + config.theta_search_time * avg_search_time
            + config.theta_blowout * blowout_rate
            + config.theta_win_rate * win_rate
            + config.theta_performance * avg_performance;
        
        // Logistic function: σ(x) = 1 / (1 + exp(-x))
        let prob = 1.0 / (1.0 + (-logit).exp());
        
        // Validate and clamp
        let prob = prob.clamp(0.0, 1.0);
        if prob.is_finite() {
            prob
        } else {
            // Fallback to neutral probability if calculation fails
            0.5
        }
    }

    /// Per whitepaper §3.8: P(continue) = σ(θ^T z_i)
    /// where z_i is the experience vector and θ are the retention coefficients
    fn compute_continue_probability(&self, player: &Player) -> f64 {
        let config = &self.config.retention_config;
        
        // If no experience history, use base probability
        if player.recent_experience.is_empty() {
            // Convert base logit to probability using logistic function
            let logit = config.base_continue_prob;
            let prob = 1.0 / (1.0 + (-logit).exp());
            return prob.clamp(0.0, 1.0);
        }
        
        // Build experience vector from recent history (last N matches)
        let window_size = config.experience_window_size.min(player.recent_experience.len());
        let recent = &player.recent_experience[player.recent_experience.len().saturating_sub(window_size)..];
        
        // Compute averages across recent matches
        let avg_delta_ping = recent.iter()
            .map(|e| e.avg_delta_ping)
            .sum::<f64>() / recent.len() as f64;
        
        let avg_search_time = recent.iter()
            .map(|e| e.avg_search_time)
            .sum::<f64>() / recent.len() as f64;
        
        let blowout_rate = recent.iter()
            .filter(|e| e.was_blowout)
            .count() as f64 / recent.len() as f64;
        
        let win_rate = recent.iter()
            .filter(|e| e.won)
            .count() as f64 / recent.len() as f64;
        
        let avg_performance = recent.iter()
            .map(|e| e.performance)
            .sum::<f64>() / recent.len() as f64;
        
        // Apply logistic model: P(continue) = σ(base + θ^T z)
        // z = [avg_delta_ping, avg_search_time, blowout_rate, win_rate, avg_performance]
        // θ = [theta_ping, theta_search_time, theta_blowout, theta_win_rate, theta_performance]
        let logit = config.base_continue_prob
            + config.theta_ping * avg_delta_ping
            + config.theta_search_time * avg_search_time
            + config.theta_blowout * blowout_rate
            + config.theta_win_rate * win_rate
            + config.theta_performance * avg_performance;
        
        // Logistic function: σ(x) = 1 / (1 + exp(-x))
        let prob = 1.0 / (1.0 + (-logit).exp());
        
        // Validate and clamp
        let prob = prob.clamp(0.0, 1.0);
        if prob.is_finite() {
            prob
        } else {
            // Fallback to neutral probability if calculation fails
            0.5
        }
    }

    /// Run a single simulation tick
    pub fn tick(&mut self) {
        let mut rng = StdRng::seed_from_u64(self.rng_seed.wrapping_add(self.current_time));

        // 1. Process arrivals (players coming online)
        self.process_arrivals(&mut rng);

        // 2. Process search starts (lobby players starting to search)
        self.process_search_starts(&mut rng);

        // 3. Run matchmaking
        let match_results = self.run_matchmaking();

        // 4. Create matches from results
        self.create_matches(match_results, &mut rng);

        // 5. Process match completions
        self.process_match_completions(&mut rng);

        // 6. Update statistics
        self.update_stats();

        // 7. Advance time
        self.current_time += 1;
    }

    /// Run simulation for N ticks
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Update simulation statistics
    fn update_stats(&mut self) {
        self.stats.time_elapsed = self.current_time as f64 * self.config.tick_interval;
        self.stats.ticks = self.current_time;
        self.stats.skill_evolution_enabled = self.config.enable_skill_evolution;
        
        // Count players by state
        self.stats.players_offline = 0;
        self.stats.players_in_lobby = 0;
        self.stats.players_searching = 0;
        self.stats.players_in_match = 0;
        
        for player in self.players.values() {
            match player.state {
                PlayerState::Offline => self.stats.players_offline += 1,
                PlayerState::InLobby => self.stats.players_in_lobby += 1,
                PlayerState::Searching => self.stats.players_searching += 1,
                PlayerState::InMatch => self.stats.players_in_match += 1,
            }
        }
        
        self.stats.active_matches = self.matches.len();
        
        // Calculate percentiles
        if !self.stats.search_time_samples.is_empty() {
            let mut sorted = self.stats.search_time_samples.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            self.stats.avg_search_time = sorted.iter().sum::<f64>() / sorted.len() as f64;
            self.stats.search_time_p50 = sorted[sorted.len() / 2];
            self.stats.search_time_p90 = sorted[(sorted.len() as f64 * 0.9) as usize];
            self.stats.search_time_p99 = sorted[(sorted.len() as f64 * 0.99).min((sorted.len() - 1) as f64) as usize];
        }
        
        if !self.stats.delta_ping_samples.is_empty() {
            let mut sorted = self.stats.delta_ping_samples.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            self.stats.avg_delta_ping = sorted.iter().sum::<f64>() / sorted.len() as f64;
            self.stats.delta_ping_p50 = sorted[sorted.len() / 2];
            self.stats.delta_ping_p90 = sorted[(sorted.len() as f64 * 0.9) as usize];
        }
        
        if !self.stats.skill_disparity_samples.is_empty() {
            self.stats.avg_skill_disparity = self.stats.skill_disparity_samples.iter().sum::<f64>() 
                / self.stats.skill_disparity_samples.len() as f64;
        }
        
        // Blowout rate
        if self.stats.total_matches > 0 {
            self.stats.blowout_rate = self.stats.blowout_count as f64 / self.stats.total_matches as f64;
        }
        
        // Calculate per-playlist blowout rates
        self.stats.per_playlist_blowout_rate.clear();
        for (playlist, &match_count) in &self.stats.per_playlist_match_counts {
            if match_count > 0 {
                let blowout_count = self.stats.per_playlist_blowout_counts.get(playlist).copied().unwrap_or(0);
                let rate = blowout_count as f64 / match_count as f64;
                self.stats.per_playlist_blowout_rate.insert(*playlist, rate);
            }
        }
        
        // Calculate per-bucket statistics
        self.update_bucket_stats();
        
        // Calculate retention metrics
        self.update_retention_stats();
        
        // Calculate return probability and churn metrics
        self.update_return_stats();
        self.update_churn_stats();
        self.update_leaving_rate();
        self.update_population_change_rate();
        
        // Track effective population size over time (sample every 10 ticks to avoid excessive memory)
        if self.current_time % 10 == 0 {
            let effective_population = self.stats.players_in_lobby 
                + self.stats.players_searching 
                + self.stats.players_in_match;
            self.stats.effective_population_size_over_time.push((self.current_time, effective_population));
        }
        
        // Calculate party statistics
        self.stats.party_count = self.parties.len();
        if !self.parties.is_empty() {
            let total_party_size: usize = self.parties.values().map(|p| p.size()).sum();
            self.stats.avg_party_size = total_party_size as f64 / self.parties.len() as f64;
        } else {
            self.stats.avg_party_size = 0.0;
        }
    }

    /// Update return probability statistics
    fn update_return_stats(&mut self) {
        // Calculate per-bucket return rates
        self.stats.per_bucket_return_rate.clear();
        for (bucket, &attempts) in &self.return_attempts_by_bucket {
            let returns = self.returns_by_bucket.get(bucket).copied().unwrap_or(0);
            if attempts > 0 {
                let rate = returns as f64 / attempts as f64;
                self.stats.per_bucket_return_rate.insert(*bucket, rate);
            }
        }
    }

    /// Update players leaving rate (quits per second)
    /// Note: This is normalized by match completion rate to avoid feedback loop
    /// where fewer players → fewer matches → fewer continuation decisions → lower quit rate
    fn update_leaving_rate(&mut self) {
        // Keep only recent quits (last 100 ticks for rolling average)
        let cutoff_tick = self.current_time.saturating_sub(100);
        self.stats.recent_quits.retain(|(tick, _)| *tick >= cutoff_tick);
        
        // Calculate total quits in recent window
        let total_quits: usize = self.stats.recent_quits.iter().map(|(_, count)| count).sum();
        let time_window_seconds = 100.0 * self.config.tick_interval;
        
        // Calculate rate (quits per second)
        // This is the raw rate - it will be lower when fewer matches complete
        // The diagnostic "Avg Continue Prob" shows the actual quit probability per decision
        if time_window_seconds > 0.0 {
            self.stats.players_leaving_rate = total_quits as f64 / time_window_seconds;
        } else {
            self.stats.players_leaving_rate = 0.0;
        }
    }

    /// Update population change rate (players per second)
    /// Tracks the rate at which effective (active) population is changing
    /// Effective population = players in lobby + searching + in match
    fn update_population_change_rate(&mut self) {
        // Calculate effective population (active players)
        let effective_population = self.stats.players_in_lobby 
            + self.stats.players_searching 
            + self.stats.players_in_match;
        
        // Record current effective population
        self.stats.population_history.push((self.current_time, effective_population));
        
        // Keep diagnostic samples (last 10)
        self.stats.recent_population_samples.push(effective_population);
        if self.stats.recent_population_samples.len() > 10 {
            self.stats.recent_population_samples.remove(0);
        }
        
        // Keep only last 200 ticks for better trend calculation
        if self.stats.population_history.len() > 200 {
            self.stats.population_history.remove(0);
        }
        
        // Calculate rate of change (first derivative)
        // Use simple difference for more responsive calculation
        if self.stats.population_history.len() >= 2 {
            // Use last 30-50 points for smoother estimate
            let history_len = self.stats.population_history.len();
            let window_size = history_len.min(50).max(10);
            // Ensure we don't underflow when calculating the start index
            let start_idx = if history_len >= window_size {
                history_len - window_size
            } else {
                0
            };
            let recent = &self.stats.population_history[start_idx..];
            
            if recent.len() >= 2 {
                // Simple approach: calculate average rate over the window
                let first = recent[0];
                let last = recent[recent.len() - 1];
                let time_diff_ticks = last.0.saturating_sub(first.0);
                
                // Ensure we have at least 1 tick difference to avoid division by zero
                if time_diff_ticks > 0 {
                    let time_diff_seconds = time_diff_ticks as f64 * self.config.tick_interval;
                    let pop_diff = last.1 as f64 - first.1 as f64;
                    
                    // Calculate rate: change in population / change in time (in seconds)
                    self.stats.population_change_rate = pop_diff / time_diff_seconds;
                } else {
                    // If no time difference, use 0
                    self.stats.population_change_rate = 0.0;
                }
            } else {
                self.stats.population_change_rate = 0.0;
            }
        } else {
            self.stats.population_change_rate = 0.0;
        }
    }

    /// Update churn statistics
    /// Churn rate represents the fraction of total population that is currently churned
    /// (offline for longer than threshold and haven't returned)
    fn update_churn_stats(&mut self) {
        let threshold = self.stats.churn_threshold_ticks;
        let mut churned_players = 0;
        let total_population = self.players.len();
        
        for player in self.players.values() {
            if player.state == PlayerState::Offline {
                if let Some(last_end_time) = player.last_session_end_time {
                    let time_since_offline = self.current_time.saturating_sub(last_end_time);
                    if time_since_offline > threshold {
                        // Player is churned: offline for > threshold
                        churned_players += 1;
                    }
                }
            }
        }
        
        // Calculate churn rate as fraction of total population
        // This represents how much of the population is currently churned
        if total_population > 0 {
            self.stats.churn_rate = churned_players as f64 / total_population as f64;
        } else {
            self.stats.churn_rate = 0.0;
        }
    }

    /// Update retention and session statistics
    fn update_retention_stats(&mut self) {
        // Calculate per-bucket continuation rates
        self.stats.per_bucket_continue_rate.clear();
        for (bucket, &(continues, quits)) in &self.session_continues {
            let total = continues + quits;
            if total > 0 {
                let rate = continues as f64 / total as f64;
                self.stats.per_bucket_continue_rate.insert(*bucket, rate);
            }
        }
        
        // Calculate average matches per session
        if self.stats.total_sessions_completed > 0 {
            self.stats.avg_matches_per_session = self.total_matches_in_sessions as f64 
                / self.stats.total_sessions_completed as f64;
        } else {
            self.stats.avg_matches_per_session = 0.0;
        }
        
        // Count active sessions (players in IN_LOBBY, SEARCHING, or IN_MATCH)
        self.stats.active_sessions = self.players.values()
            .filter(|p| matches!(p.state, PlayerState::InLobby | PlayerState::Searching | PlayerState::InMatch))
            .count();
        
        // Calculate average computed continue probability (diagnostic)
        if !self.continue_prob_samples.is_empty() {
            self.stats.avg_computed_continue_prob = self.continue_prob_samples.iter().sum::<f64>() 
                / self.continue_prob_samples.len() as f64;
        } else {
            self.stats.avg_computed_continue_prob = 0.0;
        }
        
        // Store diagnostic samples
        if !self.logit_samples.is_empty() {
            self.stats.sample_logits = self.logit_samples.clone();
        }
        if !self.experience_samples.is_empty() {
            self.stats.sample_experiences = self.experience_samples.clone();
        }
        
        // Store current retention config for verification
        self.stats.current_retention_config = Some(self.config.retention_config.clone());
    }

    fn update_bucket_stats(&mut self) {
        self.stats.bucket_stats.clear();
        
        for bucket in 1..=self.config.num_skill_buckets {
            let bucket_players: Vec<&Player> = self.players
                .values()
                .filter(|p| p.skill_bucket == bucket)
                .collect();
            
            if bucket_players.is_empty() {
                continue;
            }
            
            let player_count = bucket_players.len();
            
            let avg_search_time = bucket_players.iter()
                .filter_map(|p| {
                    if p.recent_search_times.is_empty() {
                        None
                    } else {
                        Some(p.recent_search_times.iter().sum::<f64>() / p.recent_search_times.len() as f64)
                    }
                })
                .sum::<f64>() / player_count as f64;
            
            let avg_delta_ping = bucket_players.iter()
                .filter_map(|p| {
                    if p.recent_delta_pings.is_empty() {
                        None
                    } else {
                        Some(p.recent_delta_pings.iter().sum::<f64>() / p.recent_delta_pings.len() as f64)
                    }
                })
                .sum::<f64>() / player_count as f64;
            
            let total_wins: usize = bucket_players.iter().map(|p| p.wins).sum();
            let total_matches: usize = bucket_players.iter().map(|p| p.matches_played).sum();
            let win_rate = if total_matches > 0 {
                total_wins as f64 / total_matches as f64
            } else {
                0.0
            };
            
            let total_kills: usize = bucket_players.iter().map(|p| p.total_kills).sum();
            let total_deaths: usize = bucket_players.iter().map(|p| p.total_deaths).sum();
            let avg_kd = if total_deaths > 0 {
                total_kills as f64 / total_deaths as f64
            } else {
                1.0
            };
            
            self.stats.bucket_stats.insert(bucket, BucketStats {
                bucket_id: bucket,
                player_count,
                avg_search_time,
                avg_delta_ping,
                win_rate,
                avg_kd,
                matches_played: total_matches,
            });
        }
    }

    /// Poisson random sample
    fn poisson_sample(&self, lambda: f64, rng: &mut impl Rng) -> usize {
        let l = (-lambda).exp();
        let mut k = 0;
        let mut p = 1.0;
        
        loop {
            k += 1;
            p *= rng.gen::<f64>();
            if p <= l {
                break;
            }
        }
        
        k - 1
    }

    /// Get current state as JSON for frontend
    pub fn get_state_json(&self) -> String {
        serde_json::to_string(&SimulationState {
            current_time: self.current_time,
            tick_interval: self.config.tick_interval,
            total_players: self.players.len(),
            stats: self.stats.clone(),
            config: self.config.clone(),
        }).unwrap_or_default()
    }

    /// Set arrival rate
    pub fn set_arrival_rate(&mut self, rate: f64) {
        self.arrival_rate = rate;
    }

    /// Get skill distribution data
    pub fn get_skill_distribution(&self) -> Vec<(f64, usize)> {
        let mut buckets: Vec<usize> = vec![0; 20];
        
        for player in self.players.values() {
            let bucket = ((player.skill + 1.0) / 2.0 * 19.0).floor() as usize;
            let bucket = bucket.min(19);
            buckets[bucket] += 1;
        }
        
        buckets.iter().enumerate()
            .map(|(i, &count)| {
                let skill = (i as f64 / 19.0) * 2.0 - 1.0;
                (skill, count)
            })
            .collect()
    }

    /// Get all player IDs in a party
    pub fn get_party_members(&self, party_id: usize) -> Vec<usize> {
        self.parties
            .get(&party_id)
            .map(|p| p.player_ids.clone())
            .unwrap_or_default()
    }

    /// Create a new party from a list of player IDs
    pub fn create_party(&mut self, player_ids: Vec<usize>) -> Result<usize, String> {
        if player_ids.is_empty() {
            return Err("Cannot create party with no players".to_string());
        }

        // Validate all players exist and are not in other parties
        let mut party_players = Vec::new();
        for &player_id in &player_ids {
            let player = self.players.get(&player_id)
                .ok_or_else(|| format!("Player {} does not exist", player_id))?;
            
            if player.party_id.is_some() {
                return Err(format!("Player {} is already in a party", player_id));
            }

            // Validate players are in compatible states (IN_LOBBY or OFFLINE)
            if !matches!(player.state, PlayerState::InLobby | PlayerState::Offline) {
                return Err(format!("Player {} is not in a valid state to join party", player_id));
            }

            party_players.push(player);
        }

        // Validate party size (max 6 for most playlists)
        if player_ids.len() > 6 {
            return Err("Party size cannot exceed 6 players".to_string());
        }

        // Create party
        let party_id = self.next_party_id;
        self.next_party_id += 1;

        let party = Party::from_players(party_id, &party_players);

        // Set party_id for all members
        for &player_id in &player_ids {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.party_id = Some(party_id);
            }
        }

        self.parties.insert(party_id, party);
        Ok(party_id)
    }

    /// Add a player to an existing party
    pub fn join_party(&mut self, party_id: usize, player_id: usize) -> Result<(), String> {
        let party = self.parties.get_mut(&party_id)
            .ok_or_else(|| format!("Party {} does not exist", party_id))?;

        // Validate party capacity (max 6)
        if party.size() >= 6 {
            return Err("Party is at maximum capacity".to_string());
        }

        // Validate player exists and is not in another party
        let player = self.players.get(&player_id)
            .ok_or_else(|| format!("Player {} does not exist", player_id))?;

        if player.party_id.is_some() {
            return Err(format!("Player {} is already in a party", player_id));
        }

        // Validate player is in compatible state
        if !matches!(player.state, PlayerState::InLobby | PlayerState::Offline) {
            return Err(format!("Player {} is not in a valid state to join party", player_id));
        }

        // Add player to party
        party.player_ids.push(player_id);
        
        // Update player's party_id
        if let Some(player) = self.players.get_mut(&player_id) {
            player.party_id = Some(party_id);
        }

        // Update party aggregates
        party.update_aggregates(&self.players);
        Ok(())
    }

    /// Remove a player from a party
    pub fn leave_party(&mut self, party_id: usize, player_id: usize) -> Result<(), String> {
        let party = self.parties.get_mut(&party_id)
            .ok_or_else(|| format!("Party {} does not exist", party_id))?;

        // Validate player is a member
        if !party.player_ids.contains(&player_id) {
            return Err(format!("Player {} is not a member of party {}", player_id, party_id));
        }

        // Remove player from party
        party.player_ids.retain(|&id| id != player_id);

        // Clear player's party_id
        if let Some(player) = self.players.get_mut(&player_id) {
            player.party_id = None;
        }

        // If party becomes empty, disband it
        if party.player_ids.is_empty() {
            self.parties.remove(&party_id);
            return Ok(());
        }

        // If leader left, assign new leader (next player in list)
        if party.leader_id == player_id {
            party.leader_id = party.player_ids[0];
        }

        // Update party aggregates
        party.update_aggregates(&self.players);
        Ok(())
    }

    /// Disband a party completely
    pub fn disband_party(&mut self, party_id: usize) -> Result<(), String> {
        let party = self.parties.remove(&party_id)
            .ok_or_else(|| format!("Party {} does not exist", party_id))?;

        // Clear party_id for all members
        for &player_id in &party.player_ids {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.party_id = None;
                
                // If any members were searching, remove their search objects
                if player.state == PlayerState::Searching {
                    player.state = PlayerState::InLobby;
                    // Remove search objects containing this player
                    self.searches.retain(|s| !s.player_ids.contains(&player_id));
                }
            }
        }

        Ok(())
    }

    /// Update config parameter
    pub fn update_config(&mut self, config: MatchmakingConfig) {
        self.config = config;
    }
}

#[derive(Serialize, Deserialize)]
pub struct SimulationState {
    pub current_time: u64,
    pub tick_interval: f64,
    pub total_players: usize,
    pub stats: SimulationStats,
    pub config: MatchmakingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_simulation() -> Simulation {
        let config = MatchmakingConfig::default();
        let mut sim = Simulation::new(config, 42);
        sim.init_default_data_centers();
        sim.generate_population(100, None);
        sim
    }

    #[test]
    fn test_create_party() {
        let mut sim = create_test_simulation();
        
        // Get some players
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        // Set players to InLobby state
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        // Create party
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        
        // Verify party exists
        assert!(sim.parties.contains_key(&party_id));
        
        // Verify all players have party_id set
        for &pid in &player_ids {
            assert_eq!(sim.players.get(&pid).unwrap().party_id, Some(party_id));
        }
        
        // Verify party has correct members
        let party = sim.parties.get(&party_id).unwrap();
        assert_eq!(party.player_ids.len(), 3);
        assert_eq!(party.leader_id, player_ids[0]);
    }

    #[test]
    fn test_create_party_duplicate_player() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(2).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        // Create first party
        sim.create_party(player_ids.clone()).unwrap();
        
        // Try to create another party with same player - should fail
        let result = sim.create_party(vec![player_ids[0], player_ids[1]]);
        assert!(result.is_err());
    }

    #[test]
    fn test_join_party() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(4).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        // Create party with first 2 players
        let party_id = sim.create_party(player_ids[0..2].to_vec()).unwrap();
        
        // Join third player
        sim.join_party(party_id, player_ids[2]).unwrap();
        
        let party = sim.parties.get(&party_id).unwrap();
        assert_eq!(party.player_ids.len(), 3);
        assert_eq!(sim.players.get(&player_ids[2]).unwrap().party_id, Some(party_id));
    }

    #[test]
    fn test_leave_party() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        let leader_id = player_ids[0];
        
        // Leave party (non-leader)
        sim.leave_party(party_id, player_ids[1]).unwrap();
        
        let party = sim.parties.get(&party_id).unwrap();
        assert_eq!(party.player_ids.len(), 2);
        assert_eq!(sim.players.get(&player_ids[1]).unwrap().party_id, None);
        
        // Leader should still be leader
        assert_eq!(party.leader_id, leader_id);
    }

    #[test]
    fn test_leave_party_leader_reassignment() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        let old_leader = player_ids[0];
        let new_leader = player_ids[1];
        
        // Leader leaves
        sim.leave_party(party_id, old_leader).unwrap();
        
        let party = sim.parties.get(&party_id).unwrap();
        // New leader should be next player in list
        assert_eq!(party.leader_id, new_leader);
    }

    #[test]
    fn test_disband_party() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        
        // Disband party
        sim.disband_party(party_id).unwrap();
        
        // Party should be removed
        assert!(!sim.parties.contains_key(&party_id));
        
        // All players should have party_id cleared
        for &pid in &player_ids {
            assert_eq!(sim.players.get(&pid).unwrap().party_id, None);
        }
    }

    #[test]
    fn test_party_search_creates_single_search_object() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        for &pid in &player_ids {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.state = PlayerState::InLobby;
            }
        }
        
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        
        // Start search (leader starts)
        sim.start_search(player_ids[0]);
        
        // Should have exactly one search object with all party members
        assert_eq!(sim.searches.len(), 1);
        let search = &sim.searches[0];
        assert_eq!(search.player_ids.len(), 3);
        assert_eq!(search.player_ids, player_ids);
        
        // All party members should be searching
        for &pid in &player_ids {
            assert_eq!(sim.players.get(&pid).unwrap().state, PlayerState::Searching);
        }
    }

    #[test]
    fn test_party_aggregates() {
        let mut sim = create_test_simulation();
        let player_ids: Vec<usize> = sim.players.keys().take(3).copied().collect();
        
        // Set different skills for players
        let skills = vec![0.5, 0.3, 0.7];
        for (i, &pid) in player_ids.iter().enumerate() {
            if let Some(player) = sim.players.get_mut(&pid) {
                player.skill = skills[i];
                player.skill_percentile = skills[i];
                player.state = PlayerState::InLobby;
            }
        }
        
        sim.update_skill_percentiles();
        
        let party_id = sim.create_party(player_ids.clone()).unwrap();
        let party = sim.parties.get(&party_id).unwrap();
        
        // Verify aggregates
        let expected_avg = (0.5 + 0.3 + 0.7) / 3.0;
        assert!((party.avg_skill - expected_avg).abs() < 0.001);
        
        let expected_disparity = 0.7 - 0.3;
        assert!((party.skill_disparity - expected_disparity).abs() < 0.001);
    }
}
