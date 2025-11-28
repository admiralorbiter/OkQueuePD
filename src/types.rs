use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Geographic coordinates (latitude, longitude)
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
}

impl Location {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }

    /// Haversine distance in kilometers
    pub fn distance_km(&self, other: &Location) -> f64 {
        let r = 6371.0; // Earth radius in km
        let d_lat = (other.lat - self.lat).to_radians();
        let d_lon = (other.lon - self.lon).to_radians();
        let lat1 = self.lat.to_radians();
        let lat2 = other.lat.to_radians();

        let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        r * c
    }
}

/// Platform types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    PC,
    PlayStation,
    Xbox,
}

/// Input device types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputDevice {
    Controller,
    MouseKeyboard,
}

/// Player activity state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Offline,
    InLobby,
    Searching,
    InMatch,
}

/// Available playlists/game modes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Playlist {
    TeamDeathmatch,      // 6v6
    SearchAndDestroy,    // 6v6
    Domination,          // 6v6
    GroundWar,           // 32v32
    FreeForAll,          // 12 players
}

impl Playlist {
    pub fn required_players(&self) -> usize {
        match self {
            Playlist::TeamDeathmatch => 12,
            Playlist::SearchAndDestroy => 12,
            Playlist::Domination => 12,
            Playlist::GroundWar => 64,
            Playlist::FreeForAll => 12,
        }
    }

    pub fn team_count(&self) -> usize {
        match self {
            Playlist::FreeForAll => 12,
            Playlist::GroundWar => 2,
            _ => 2,
        }
    }

    pub fn avg_match_duration_seconds(&self) -> f64 {
        match self {
            Playlist::TeamDeathmatch => 600.0,      // 10 min
            Playlist::SearchAndDestroy => 900.0,    // 15 min
            Playlist::Domination => 600.0,          // 10 min
            Playlist::GroundWar => 1200.0,          // 20 min
            Playlist::FreeForAll => 600.0,          // 10 min
        }
    }
}

/// Data center information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataCenter {
    pub id: usize,
    pub name: String,
    pub location: Location,
    pub region: String,
    /// Server capacity per playlist
    pub server_capacity: HashMap<Playlist, usize>,
    /// Currently busy servers per playlist
    pub busy_servers: HashMap<Playlist, usize>,
}

impl DataCenter {
    pub fn new(id: usize, name: &str, location: Location, region: &str) -> Self {
        let mut server_capacity = HashMap::new();
        let mut busy_servers = HashMap::new();
        
        // Default capacities
        for playlist in [
            Playlist::TeamDeathmatch,
            Playlist::SearchAndDestroy,
            Playlist::Domination,
            Playlist::GroundWar,
            Playlist::FreeForAll,
        ] {
            let capacity = match playlist {
                Playlist::GroundWar => 50,
                _ => 200,
            };
            server_capacity.insert(playlist, capacity);
            busy_servers.insert(playlist, 0);
        }

        Self {
            id,
            name: name.to_string(),
            location,
            region: region.to_string(),
            server_capacity,
            busy_servers,
        }
    }

    pub fn available_servers(&self, playlist: &Playlist) -> usize {
        let capacity = self.server_capacity.get(playlist).copied().unwrap_or(0);
        let busy = self.busy_servers.get(playlist).copied().unwrap_or(0);
        capacity.saturating_sub(busy)
    }
}

/// Player statistics and state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: usize,
    pub location: Location,
    pub platform: Platform,
    pub input_device: InputDevice,
    pub voice_chat_enabled: bool,
    
    /// Raw skill value in [-1, 1]
    pub skill: f64,
    /// Skill percentile in [0, 1]
    pub skill_percentile: f64,
    /// Skill bucket (1 to B)
    pub skill_bucket: usize,
    
    /// Current state
    pub state: PlayerState,
    /// Current match ID if in match
    pub current_match: Option<usize>,
    /// Current party ID
    pub party_id: Option<usize>,
    
    /// Preferred playlists (Quick Play set)
    pub preferred_playlists: HashSet<Playlist>,
    
    /// Ping to each data center (DC id -> ping in ms)
    pub dc_pings: HashMap<usize, f64>,
    /// Best data center ID
    pub best_dc: Option<usize>,
    /// Best ping value
    pub best_ping: f64,
    
    /// Search start time (simulation ticks)
    pub search_start_time: Option<u64>,
    
    /// Session statistics
    pub matches_played: usize,
    pub total_kills: usize,
    pub total_deaths: usize,
    pub wins: usize,
    pub losses: usize,
    
    /// Recent experience metrics (for quit probability)
    pub recent_delta_pings: Vec<f64>,
    pub recent_search_times: Vec<f64>,
    pub recent_blowouts: Vec<bool>,
    
    /// Recent performance indices from matches (rolling window)
    pub recent_performance: Vec<f64>,
    
    /// Continuation probability (search again after match) - kept for backward compatibility
    /// Will be computed dynamically using retention model
    pub continuation_prob: f64,
    
    /// Recent experience vectors (last N matches) for retention model
    pub recent_experience: Vec<ExperienceVector>,
    
    /// Session tracking: when current session started (tick)
    pub session_start_time: Option<u64>,
    
    /// Session tracking: number of matches played in current session
    pub matches_in_session: usize,
    
    /// Return probability tracking: experience from last completed session
    pub last_session_experience: Vec<ExperienceVector>,
    
    /// Return probability tracking: when player last went offline (tick)
    pub last_session_end_time: Option<u64>,
}

impl Player {
    pub fn new(id: usize, location: Location, skill: f64) -> Self {
        let mut preferred = HashSet::new();
        preferred.insert(Playlist::TeamDeathmatch);
        
        Self {
            id,
            location,
            platform: Platform::PC,
            input_device: InputDevice::Controller,
            voice_chat_enabled: true,
            skill,
            skill_percentile: 0.5,
            skill_bucket: 5,
            state: PlayerState::Offline,
            current_match: None,
            party_id: None,
            preferred_playlists: preferred,
            dc_pings: HashMap::new(),
            best_dc: None,
            best_ping: 1000.0,
            search_start_time: None,
            matches_played: 0,
            total_kills: 0,
            total_deaths: 0,
            wins: 0,
            losses: 0,
            recent_delta_pings: Vec::new(),
            recent_search_times: Vec::new(),
            recent_blowouts: Vec::new(),
            recent_performance: Vec::new(),
            continuation_prob: 0.85,
            recent_experience: Vec::new(),
            session_start_time: None,
            matches_in_session: 0,
            last_session_experience: Vec::new(),
            last_session_end_time: None,
        }
    }

    /// Calculate acceptable data centers based on wait time
    pub fn acceptable_dcs(&self, wait_time: f64, config: &MatchmakingConfig) -> Vec<usize> {
        let delta_ping_allowed = config.delta_ping_backoff(wait_time);
        
        self.dc_pings
            .iter()
            .filter(|(_, &ping)| {
                ping <= self.best_ping + delta_ping_allowed && ping <= config.max_ping
            })
            .map(|(&dc_id, _)| dc_id)
            .collect()
    }

    /// Update skill bucket based on percentile
    pub fn update_skill_bucket(&mut self, num_buckets: usize) {
        self.skill_bucket = ((self.skill_percentile * num_buckets as f64).floor() as usize)
            .clamp(1, num_buckets);
    }
}

/// A party of players searching together
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Party {
    pub id: usize,
    pub player_ids: Vec<usize>,
    pub leader_id: usize,
    /// Average skill of party (raw skill)
    pub avg_skill: f64,
    /// Skill disparity within party (raw skill)
    pub skill_disparity: f64,
    /// Average skill percentile
    pub avg_skill_percentile: f64,
    /// Skill percentile disparity
    pub skill_percentile_disparity: f64,
    /// Preferred playlists (intersection of all party members' preferences)
    pub preferred_playlists: HashSet<Playlist>,
    /// Platform composition (platform -> count)
    pub platforms: HashMap<Platform, usize>,
    /// Input device composition (input device -> count)
    pub input_devices: HashMap<InputDevice, usize>,
    /// Average location (centroid of party member locations)
    pub avg_location: Location,
}

impl Party {
    pub fn size(&self) -> usize {
        self.player_ids.len()
    }

    /// Construct a party from player references, computing all aggregates
    /// Per whitepaper §2.4: avg_skill, skill_disparity, avg_percentile, percentile_disparity
    pub fn from_players(id: usize, players: &[&Player]) -> Self {
        if players.is_empty() {
            panic!("Cannot create party from empty player list");
        }

        let player_ids: Vec<usize> = players.iter().map(|p| p.id).collect();
        let leader_id = players[0].id;

        // Compute skill aggregates (raw skill)
        let skills: Vec<f64> = players.iter().map(|p| p.skill).collect();
        let avg_skill = skills.iter().sum::<f64>() / skills.len() as f64;
        let skill_disparity = {
            let min_skill = skills.iter().fold(f64::MAX, |a, &b| a.min(b));
            let max_skill = skills.iter().fold(f64::MIN, |a, &b| a.max(b));
            max_skill - min_skill
        };

        // Compute percentile aggregates
        let percentiles: Vec<f64> = players.iter().map(|p| p.skill_percentile).collect();
        let avg_skill_percentile = percentiles.iter().sum::<f64>() / percentiles.len() as f64;
        let skill_percentile_disparity = {
            let min_percentile = percentiles.iter().fold(f64::MAX, |a, &b| a.min(b));
            let max_percentile = percentiles.iter().fold(f64::MIN, |a, &b| a.max(b));
            max_percentile - min_percentile
        };

        // Compute preferred playlists (intersection of all members)
        let mut preferred_playlists = players[0].preferred_playlists.clone();
        for player in players.iter().skip(1) {
            preferred_playlists = preferred_playlists
                .intersection(&player.preferred_playlists)
                .copied()
                .collect();
        }

        // Compute platform composition
        let mut platforms = HashMap::new();
        for player in players {
            *platforms.entry(player.platform).or_insert(0) += 1;
        }

        // Compute input device composition
        let mut input_devices = HashMap::new();
        for player in players {
            *input_devices.entry(player.input_device).or_insert(0) += 1;
        }

        // Compute average location (centroid)
        let avg_location = {
            let total_lat: f64 = players.iter().map(|p| p.location.lat).sum();
            let total_lon: f64 = players.iter().map(|p| p.location.lon).sum();
            let count = players.len() as f64;
            Location::new(total_lat / count, total_lon / count)
        };

        Self {
            id,
            player_ids,
            leader_id,
            avg_skill,
            skill_disparity,
            avg_skill_percentile,
            skill_percentile_disparity,
            preferred_playlists,
            platforms,
            input_devices,
            avg_location,
        }
    }

    /// Recompute aggregates when party membership changes
    pub fn update_aggregates(&mut self, players: &HashMap<usize, Player>) {
        let party_players: Vec<&Player> = self.player_ids
            .iter()
            .filter_map(|id| players.get(id))
            .collect();

        if party_players.is_empty() {
            return;
        }

        // Update skill aggregates (raw skill)
        let skills: Vec<f64> = party_players.iter().map(|p| p.skill).collect();
        self.avg_skill = skills.iter().sum::<f64>() / skills.len() as f64;
        self.skill_disparity = {
            let min_skill = skills.iter().fold(f64::MAX, |a, &b| a.min(b));
            let max_skill = skills.iter().fold(f64::MIN, |a, &b| a.max(b));
            max_skill - min_skill
        };

        // Update percentile aggregates
        let percentiles: Vec<f64> = party_players.iter().map(|p| p.skill_percentile).collect();
        self.avg_skill_percentile = percentiles.iter().sum::<f64>() / percentiles.len() as f64;
        self.skill_percentile_disparity = {
            let min_percentile = percentiles.iter().fold(f64::MAX, |a, &b| a.min(b));
            let max_percentile = percentiles.iter().fold(f64::MIN, |a, &b| a.max(b));
            max_percentile - min_percentile
        };

        // Update preferred playlists (intersection)
        if let Some(first_player) = party_players.first() {
            self.preferred_playlists = first_player.preferred_playlists.clone();
            for player in party_players.iter().skip(1) {
                self.preferred_playlists = self.preferred_playlists
                    .intersection(&player.preferred_playlists)
                    .copied()
                    .collect();
            }
        }

        // Update platform composition
        self.platforms.clear();
        for player in &party_players {
            *self.platforms.entry(player.platform).or_insert(0) += 1;
        }

        // Update input device composition
        self.input_devices.clear();
        for player in &party_players {
            *self.input_devices.entry(player.input_device).or_insert(0) += 1;
        }

        // Update average location (centroid)
        let total_lat: f64 = party_players.iter().map(|p| p.location.lat).sum();
        let total_lon: f64 = party_players.iter().map(|p| p.location.lon).sum();
        let count = party_players.len() as f64;
        self.avg_location = Location::new(total_lat / count, total_lon / count);
    }

    /// Convert party to SearchObject with proper DC intersection
    pub fn to_search_object(
        &self,
        search_id: usize,
        search_start_time: u64,
        players: &HashMap<usize, Player>,
        config: &MatchmakingConfig,
    ) -> SearchObject {
        let party_players: Vec<&Player> = self.player_ids
            .iter()
            .filter_map(|id| players.get(id))
            .collect();

        // Compute acceptable DCs as intersection of all party members' acceptable DCs
        let wait_time = 0.0; // Initial wait time when search starts
        let mut acceptable_dcs: Option<HashSet<usize>> = None;
        
        for player in &party_players {
            let player_dcs: HashSet<usize> = player
                .acceptable_dcs(wait_time, config)
                .into_iter()
                .collect();
            
            acceptable_dcs = Some(match acceptable_dcs {
                None => player_dcs,
                Some(existing) => existing.intersection(&player_dcs).copied().collect(),
            });
        }

        SearchObject {
            id: search_id,
            player_ids: self.player_ids.clone(),
            avg_skill_percentile: self.avg_skill_percentile,
            skill_disparity: self.skill_percentile_disparity,
            avg_location: self.avg_location,
            platforms: self.platforms.clone(),
            input_devices: self.input_devices.clone(),
            acceptable_playlists: self.preferred_playlists.clone(),
            search_start_time,
            acceptable_dcs: acceptable_dcs.unwrap_or_default(),
        }
    }
}

/// A search object (party or partial lobby in queue)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchObject {
    pub id: usize,
    pub player_ids: Vec<usize>,
    /// Average skill percentile
    pub avg_skill_percentile: f64,
    /// Skill disparity
    pub skill_disparity: f64,
    /// Average location
    pub avg_location: Location,
    /// Platform composition
    pub platforms: HashMap<Platform, usize>,
    /// Input device composition
    pub input_devices: HashMap<InputDevice, usize>,
    /// Acceptable playlists (intersection of player preferences)
    pub acceptable_playlists: HashSet<Playlist>,
    /// Search start time
    pub search_start_time: u64,
    /// Currently acceptable data centers
    pub acceptable_dcs: HashSet<usize>,
}

impl SearchObject {
    pub fn size(&self) -> usize {
        self.player_ids.len()
    }
    
    pub fn wait_time(&self, current_time: u64, tick_interval: f64) -> f64 {
        ((current_time - self.search_start_time) as f64) * tick_interval
    }
}

/// Blowout severity classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlowoutSeverity {
    Mild,      // Minor skill imbalance
    Moderate,  // Noticeable skill difference
    Severe,    // Significant skill gap
}

/// Experience vector for a single match
/// Per whitepaper §3.8: z_i = (Δp_i, T^search_i, blowout flag, KPM_i, placement percentile, ...)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperienceVector {
    /// Average delta ping in this match (ms)
    pub avg_delta_ping: f64,
    /// Search time for this match (seconds)
    pub avg_search_time: f64,
    /// Whether this match was a blowout
    pub was_blowout: bool,
    /// Whether player won this match
    pub won: bool,
    /// Performance index from match (0-1 scale)
    pub performance: f64,
}

/// Retention model configuration
/// Per whitepaper §3.8: P(continue) = σ(θ^T z_i)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Coefficient for delta ping (typically negative: high ping reduces retention)
    pub theta_ping: f64,
    /// Coefficient for search time (typically negative: long waits reduce retention)
    pub theta_search_time: f64,
    /// Coefficient for blowout rate (typically negative: blowouts reduce retention)
    pub theta_blowout: f64,
    /// Coefficient for win rate (typically positive: winning increases retention)
    pub theta_win_rate: f64,
    /// Coefficient for performance (typically positive: good performance increases retention)
    pub theta_performance: f64,
    /// Base logit (before experience terms) - maps to base probability via logistic
    pub base_continue_prob: f64,
    /// How many recent matches to include in experience vector
    pub experience_window_size: usize,
}

/// An active match
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Match {
    pub id: usize,
    pub playlist: Playlist,
    pub data_center_id: usize,
    /// Teams: team index -> player IDs
    pub teams: Vec<Vec<usize>>,
    /// Start time
    pub start_time: u64,
    /// Expected duration in simulation ticks
    pub expected_duration: u64,
    /// Team skills (for outcome prediction)
    pub team_skills: Vec<f64>,
    /// Match quality score
    pub quality_score: f64,
    /// Skill disparity across all players
    pub skill_disparity: f64,
    /// Average delta ping
    pub avg_delta_ping: f64,
    /// Expected score differential based on skill
    pub expected_score_differential: f64,
    /// Win probability imbalance (0-1 scale, how one-sided the match is)
    pub win_probability_imbalance: f64,
    /// Blowout severity classification if blowout occurs
    pub blowout_severity: Option<BlowoutSeverity>,
    /// Performance index per player (player_id -> performance)
    pub player_performances: HashMap<usize, f64>,
}

/// Matchmaking configuration parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchmakingConfig {
    /// Maximum acceptable ping (ms)
    pub max_ping: f64,
    /// Delta ping backoff curve parameters
    pub delta_ping_initial: f64,
    pub delta_ping_rate: f64,
    pub delta_ping_max: f64,
    
    /// Skill backoff curve parameters
    pub skill_similarity_initial: f64,
    pub skill_similarity_rate: f64,
    pub skill_similarity_max: f64,
    
    /// Maximum skill disparity
    pub max_skill_disparity_initial: f64,
    pub max_skill_disparity_rate: f64,
    pub max_skill_disparity_max: f64,
    
    /// Distance metric weights
    pub weight_geo: f64,
    pub weight_skill: f64,
    pub weight_input: f64,
    pub weight_platform: f64,
    
    /// Quality score weights
    pub quality_weight_ping: f64,
    pub quality_weight_skill_balance: f64,
    pub quality_weight_wait_time: f64,
    
    /// Fraction of players that participate in parties (0.0 - 1.0)
    /// This controls the baseline solo vs party mix in the simulation.
    /// Parties are auto-generated from the population using this target fraction.
    pub party_player_fraction: f64,
    
    /// Matchmaking tick interval (seconds)
    pub tick_interval: f64,
    
    /// Number of skill buckets
    pub num_skill_buckets: usize,
    
    /// Top K candidates to consider per seed
    pub top_k_candidates: usize,
    
    /// Enable expensive exact balancing for small modes (6v6)
    pub use_exact_team_balancing: bool,
    /// Logistic coefficient for win probability calculation
    pub gamma: f64,
    /// Weight for skill difference in blowout detection
    pub blowout_skill_coefficient: f64,
    /// Weight for win-probability imbalance in blowout detection
    pub blowout_imbalance_coefficient: f64,
    /// Threshold for mild blowouts
    pub blowout_mild_threshold: f64,
    /// Threshold for moderate blowouts
    pub blowout_moderate_threshold: f64,
    /// Threshold for severe blowouts
    pub blowout_severe_threshold: f64,
    
    /// Skill learning rate (α in update rule: s_i^+ = s_i^- + α(ŷ_i - E[Y_i]))
    pub skill_learning_rate: f64,
    /// Performance noise standard deviation (σ for ε_i ~ N(0, σ²))
    pub performance_noise_std: f64,
    /// Enable skill evolution (false = static skill, true = evolving skill)
    pub enable_skill_evolution: bool,
    /// Update skill percentiles every N matches (batch size)
    pub skill_update_batch_size: usize,
    
    /// Retention model configuration
    pub retention_config: RetentionConfig,
}

impl Default for MatchmakingConfig {
    fn default() -> Self {
        Self {
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
            // By default, target roughly 50% of players being in parties,
            // with party sizes drawn between 2-4 members.
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
    }
}

impl MatchmakingConfig {
    /// Calculate allowed delta ping based on wait time
    pub fn delta_ping_backoff(&self, wait_time: f64) -> f64 {
        (self.delta_ping_initial + self.delta_ping_rate * wait_time)
            .min(self.delta_ping_max)
    }

    /// Calculate skill similarity tolerance based on wait time
    pub fn skill_similarity_backoff(&self, wait_time: f64) -> f64 {
        (self.skill_similarity_initial + self.skill_similarity_rate * wait_time)
            .min(self.skill_similarity_max)
    }

    /// Calculate max skill disparity based on wait time
    pub fn skill_disparity_backoff(&self, wait_time: f64) -> f64 {
        (self.max_skill_disparity_initial + self.max_skill_disparity_rate * wait_time)
            .min(self.max_skill_disparity_max)
    }
}

/// Simulation statistics for analysis
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimulationStats {
    /// Total simulation time elapsed
    pub time_elapsed: f64,
    /// Number of matchmaking ticks
    pub ticks: u64,
    
    /// Total matches created
    pub total_matches: usize,
    /// Active matches
    pub active_matches: usize,
    
    /// Players by state
    pub players_offline: usize,
    pub players_in_lobby: usize,
    pub players_searching: usize,
    pub players_in_match: usize,
    
    /// Search time statistics (seconds)
    pub avg_search_time: f64,
    pub search_time_p50: f64,
    pub search_time_p90: f64,
    pub search_time_p99: f64,
    pub search_time_samples: Vec<f64>,
    
    /// Delta ping statistics (ms)
    pub avg_delta_ping: f64,
    pub delta_ping_p50: f64,
    pub delta_ping_p90: f64,
    pub delta_ping_samples: Vec<f64>,
    
    /// Skill disparity statistics
    pub avg_skill_disparity: f64,
    pub skill_disparity_samples: Vec<f64>,
    
    /// Match quality
    pub avg_match_quality: f64,
    
    /// Blowout rate (games with >2x score differential)
    pub blowout_rate: f64,
    pub blowout_count: usize,
    /// Blowout severity counts
    pub blowout_severity_counts: HashMap<BlowoutSeverity, usize>,
    /// Blowout rate per playlist
    pub per_playlist_blowout_rate: HashMap<Playlist, f64>,
    /// Per-playlist blowout counts (for calculating rates)
    pub per_playlist_blowout_counts: HashMap<Playlist, usize>,
    /// Per-playlist match counts (for calculating rates)
    pub per_playlist_match_counts: HashMap<Playlist, usize>,
    /// Team skill difference samples (distribution)
    pub team_skill_difference_samples: Vec<f64>,
    
    /// Per skill bucket statistics
    pub bucket_stats: HashMap<usize, BucketStats>,
    
    /// Party statistics
    pub party_count: usize,
    pub avg_party_size: f64,
    pub party_match_count: usize,
    pub solo_match_count: usize,
    pub party_search_times: Vec<f64>,
    pub solo_search_times: Vec<f64>,
    
    /// Skill evolution tracking
    /// Time series of skill distribution: (tick, [(bucket_id, mean_skill), ...])
    pub skill_distribution_over_time: Vec<(u64, Vec<(usize, f64)>)>,
    /// Whether skill evolution is currently enabled
    pub skill_evolution_enabled: bool,
    /// Total number of skill updates applied
    pub total_skill_updates: usize,
    /// Distribution of performance indices
    pub performance_samples: Vec<f64>,
    
    /// Retention model metrics
    /// Continuation rate by skill bucket (bucket_id -> continuation_rate)
    pub per_bucket_continue_rate: HashMap<usize, f64>,
    /// Average computed continue probability (diagnostic)
    pub avg_computed_continue_prob: f64,
    /// Diagnostic: sample logit values (for debugging)
    pub sample_logits: Vec<f64>,
    /// Diagnostic: sample experience values (for debugging)
    pub sample_experiences: Vec<(f64, f64, f64, f64, f64)>, // (delta_ping, search_time, blowout_rate, win_rate, performance)
    /// Diagnostic: current retention config (for verification)
    pub current_retention_config: Option<RetentionConfig>,
    /// Average matches per session
    pub avg_matches_per_session: f64,
    /// Session length distribution (histogram: index = matches, value = count)
    pub session_length_distribution: Vec<usize>,
    /// Number of players currently in a session (IN_LOBBY, SEARCHING, or IN_MATCH)
    pub active_sessions: usize,
    /// Total number of completed sessions (for calculating averages)
    pub total_sessions_completed: usize,
    
    /// Return probability and churn metrics
    /// Churn rate (fraction of total population currently offline for > threshold)
    /// Note: This is a snapshot - players can return, so churn rate can decrease
    pub churn_rate: f64,
    /// Effective population size over time (time series: (tick, concurrent_players))
    pub effective_population_size_over_time: Vec<(u64, usize)>,
    /// Return rate by skill bucket (bucket_id -> return_rate)
    pub per_bucket_return_rate: HashMap<usize, f64>,
    /// Total offline players considered for return
    pub total_return_attempts: usize,
    /// Total players who actually returned
    pub total_returns: usize,
    /// Time threshold for churn calculation (ticks)
    pub churn_threshold_ticks: u64,
    /// Players leaving rate (quits per second, rolling average)
    pub players_leaving_rate: f64,
    /// Recent quits count (for calculating leaving rate)
    pub recent_quits: Vec<(u64, usize)>, // (tick, quit_count)
    /// Population change rate (players per second, positive = growing, negative = shrinking)
    pub population_change_rate: f64,
    /// Population history for trend calculation (last 200 ticks)
    pub population_history: Vec<(u64, usize)>, // (tick, effective_population)
    /// Diagnostic: recent population values for debugging
    pub recent_population_samples: Vec<usize>, // Last 10 effective population values
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BucketStats {
    pub bucket_id: usize,
    pub player_count: usize,
    pub avg_search_time: f64,
    pub avg_delta_ping: f64,
    pub win_rate: f64,
    pub avg_kd: f64,
    pub matches_played: usize,
}

/// Research experiment configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub name: String,
    pub description: String,
    /// Parameter to vary
    pub parameter: String,
    /// Values to test
    pub values: Vec<f64>,
    /// Number of simulation runs per value
    pub runs_per_value: usize,
    /// Simulation duration per run (ticks)
    pub ticks_per_run: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_time_converts_ticks_to_seconds() {
        let search = SearchObject {
            id: 1,
            player_ids: vec![1],
            avg_skill_percentile: 0.5,
            skill_disparity: 0.0,
            avg_location: Location::new(0.0, 0.0),
            platforms: HashMap::new(),
            input_devices: HashMap::new(),
            acceptable_playlists: HashSet::new(),
            search_start_time: 0,
            acceptable_dcs: HashSet::new(),
        };
        
        let tick_interval = 5.0;
        let current_time = 10;
        
        // After 10 ticks with 5 second intervals, should be 50 seconds
        let wait_time = search.wait_time(current_time, tick_interval);
        assert_eq!(wait_time, 50.0);
    }

    #[test]
    fn test_backoff_formulas() {
        let config = MatchmakingConfig::default();
        
        // Test delta ping backoff: min(initial + rate * wait, max)
        let wait_0 = config.delta_ping_backoff(0.0);
        assert_eq!(wait_0, config.delta_ping_initial);
        
        let wait_10 = config.delta_ping_backoff(10.0);
        let expected = (config.delta_ping_initial + config.delta_ping_rate * 10.0).min(config.delta_ping_max);
        assert_eq!(wait_10, expected);
        
        // Test skill similarity backoff
        let skill_wait_0 = config.skill_similarity_backoff(0.0);
        assert_eq!(skill_wait_0, config.skill_similarity_initial);
        
        // Test skill disparity backoff
        let disparity_wait_0 = config.skill_disparity_backoff(0.0);
        assert_eq!(disparity_wait_0, config.max_skill_disparity_initial);
    }

    #[test]
    fn test_backoff_with_seconds() {
        let config = MatchmakingConfig::default();
        let tick_interval = 5.0;
        
        // Simulate 2 ticks = 10 seconds
        let wait_seconds = 2.0 * tick_interval;
        let backoff = config.delta_ping_backoff(wait_seconds);
        
        // Should use seconds, not ticks
        let expected = (config.delta_ping_initial + config.delta_ping_rate * wait_seconds).min(config.delta_ping_max);
        assert_eq!(backoff, expected);
    }
}
