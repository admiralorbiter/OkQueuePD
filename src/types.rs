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
    
    /// Continuation probability (search again after match)
    pub continuation_prob: f64,
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
            continuation_prob: 0.85,
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
    /// Average skill of party
    pub avg_skill: f64,
    /// Skill disparity within party
    pub skill_disparity: f64,
}

impl Party {
    pub fn size(&self) -> usize {
        self.player_ids.len()
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
    
    pub fn wait_time(&self, current_time: u64) -> f64 {
        (current_time - self.search_start_time) as f64
    }
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
    
    /// Matchmaking tick interval (seconds)
    pub tick_interval: f64,
    
    /// Number of skill buckets
    pub num_skill_buckets: usize,
    
    /// Top K candidates to consider per seed
    pub top_k_candidates: usize,
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
            tick_interval: 5.0,
            num_skill_buckets: 10,
            top_k_candidates: 50,
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
    
    /// Per skill bucket statistics
    pub bucket_stats: HashMap<usize, BucketStats>,
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
