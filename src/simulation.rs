use crate::matchmaker::{MatchResult, Matchmaker};
use crate::types::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    /// Random number generator seed
    rng_seed: u64,
    /// Arrival rate (players per tick)
    arrival_rate: f64,
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
            stats: SimulationStats::default(),
            next_player_id: 0,
            next_search_id: 0,
            next_match_id: 0,
            rng_seed: seed,
            arrival_rate: 10.0,
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
    }

    /// Generate skill value using a beta-like distribution
    fn generate_skill(&self, rng: &mut impl Rng) -> f64 {
        // Use sum of uniform randoms to approximate normal distribution
        let sum: f64 = (0..12).map(|_| rng.gen::<f64>()).sum();
        let normalized = (sum - 6.0) / 3.0; // Roughly N(0,1)
        normalized.clamp(-1.0, 1.0)
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

    /// Bring players online based on arrival rate
    pub fn process_arrivals(&mut self, rng: &mut impl Rng) {
        let offline_players: Vec<usize> = self.players
            .iter()
            .filter(|(_, p)| p.state == PlayerState::Offline)
            .map(|(&id, _)| id)
            .collect();

        // Poisson arrivals
        let num_arrivals = self.poisson_sample(self.arrival_rate, rng);
        let arrivals: Vec<usize> = offline_players
            .into_iter()
            .take(num_arrivals)
            .collect();

        for player_id in arrivals {
            if let Some(player) = self.players.get_mut(&player_id) {
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
            };

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
            if let Some(game_match) = self.matches.remove(&match_id) {
                // Release server
                if let Some(dc) = self.data_centers.iter_mut().find(|dc| dc.id == game_match.data_center_id) {
                    if let Some(busy) = dc.busy_servers.get_mut(&game_match.playlist) {
                        *busy = busy.saturating_sub(1);
                    }
                }

                // Determine match outcome
                let (winning_team, is_blowout) = self.determine_outcome(&game_match, rng);
                
                if is_blowout {
                    self.stats.blowout_count += 1;
                }

                // Update player stats and decide if they continue
                for (team_idx, team) in game_match.teams.iter().enumerate() {
                    let won = team_idx == winning_team;
                    
                    for &player_id in team {
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

                            // Calculate continue probability inline to avoid borrow issues
                            let base_prob = player.continuation_prob;
                            
                            let avg_delta_ping = if player.recent_delta_pings.is_empty() {
                                0.0
                            } else {
                                player.recent_delta_pings.iter().sum::<f64>() / player.recent_delta_pings.len() as f64
                            };
                            let ping_penalty = (avg_delta_ping / 100.0).min(0.2);
                            
                            let avg_search_time = if player.recent_search_times.is_empty() {
                                0.0
                            } else {
                                player.recent_search_times.iter().sum::<f64>() / player.recent_search_times.len() as f64
                            };
                            let search_penalty = (avg_search_time / 120.0).min(0.15);
                            
                            let blowout_rate = if player.recent_blowouts.is_empty() {
                                0.0
                            } else {
                                player.recent_blowouts.iter().filter(|&&b| b).count() as f64 
                                    / player.recent_blowouts.len() as f64
                            };
                            let blowout_penalty = blowout_rate * 0.2;
                            
                            let continue_prob = (base_prob - ping_penalty - search_penalty - blowout_penalty).max(0.3);

                            if rng.gen_bool(continue_prob) {
                                player.state = PlayerState::InLobby;
                            } else {
                                player.state = PlayerState::Offline;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Determine match outcome using skill difference
    fn determine_outcome(&self, game_match: &Match, rng: &mut impl Rng) -> (usize, bool) {
        if game_match.team_skills.len() < 2 {
            return (0, false);
        }

        let skill_diff = game_match.team_skills[0] - game_match.team_skills[1];
        
        // Win probability based on skill difference (logistic)
        let gamma = 2.0;
        let p_team0_wins = 1.0 / (1.0 + (-gamma * skill_diff).exp());
        
        let winning_team = if rng.gen_bool(p_team0_wins) { 0 } else { 1 };
        
        // Blowout detection: consider both skill difference and win probability
        // With balanced teams, skill differences are typically small (< 0.2)
        // So we use a lower threshold and scale probability appropriately
        let skill_diff_abs = skill_diff.abs();
        
        // Blowout probability based on:
        // 1. Skill difference (even small differences can lead to blowouts)
        // 2. Win probability imbalance (one-sided matches are more likely blowouts)
        let win_prob_imbalance = (p_team0_wins - 0.5).abs() * 2.0; // 0 to 1 scale
        
        // Base blowout probability increases with skill difference and win probability imbalance
        // Lower threshold (0.1 instead of 0.3) to catch more cases with balanced teams
        let blowout_prob = if skill_diff_abs > 0.1 {
            // Scale from 0.1 to 0.5 skill diff maps to 0.1 to 0.7 blowout probability
            let skill_component = ((skill_diff_abs - 0.1) / 0.4).min(1.0) * 0.4;
            let imbalance_component = win_prob_imbalance * 0.3;
            (0.1 + skill_component + imbalance_component).min(0.9)
        } else if win_prob_imbalance > 0.4 {
            // Even with small skill diff, high win probability imbalance suggests blowout risk
            win_prob_imbalance * 0.5
        } else {
            // Small skill diff and balanced win probability - still possible but less likely
            // Add some randomness for variance in performance
            if skill_diff_abs > 0.05 {
                0.05 + win_prob_imbalance * 0.1
            } else {
                0.02
            }
        };
        
        let is_blowout = rng.gen_bool(blowout_prob);
        
        (winning_team, is_blowout)
    }

    /// Calculate probability of player continuing based on experience
    fn calculate_continue_probability(&self, player: &Player) -> f64 {
        let base_prob = player.continuation_prob;
        
        // Penalty for high delta pings
        let avg_delta_ping = if player.recent_delta_pings.is_empty() {
            0.0
        } else {
            player.recent_delta_pings.iter().sum::<f64>() / player.recent_delta_pings.len() as f64
        };
        let ping_penalty = (avg_delta_ping / 100.0).min(0.2);
        
        // Penalty for long search times
        let avg_search_time = if player.recent_search_times.is_empty() {
            0.0
        } else {
            player.recent_search_times.iter().sum::<f64>() / player.recent_search_times.len() as f64
        };
        let search_penalty = (avg_search_time / 120.0).min(0.15);
        
        // Penalty for blowouts
        let blowout_rate = if player.recent_blowouts.is_empty() {
            0.0
        } else {
            player.recent_blowouts.iter().filter(|&&b| b).count() as f64 
                / player.recent_blowouts.len() as f64
        };
        let blowout_penalty = blowout_rate * 0.2;
        
        (base_prob - ping_penalty - search_penalty - blowout_penalty).max(0.3)
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
        
        // Calculate per-bucket statistics
        self.update_bucket_stats();
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
