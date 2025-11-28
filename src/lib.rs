mod matchmaker;
mod simulation;
mod types;

use simulation::Simulation;
use types::*;
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// WASM-exposed simulation wrapper
#[wasm_bindgen]
pub struct SimulationEngine {
    sim: Simulation,
}

#[wasm_bindgen]
impl SimulationEngine {
    /// Create a new simulation with default config
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> SimulationEngine {
        let config = MatchmakingConfig::default();
        let mut sim = Simulation::new(config, seed);
        sim.init_default_data_centers();
        SimulationEngine { sim }
    }

    /// Create with custom config
    pub fn new_with_config(seed: u64, config_json: &str) -> Result<SimulationEngine, JsValue> {
        let config: MatchmakingConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?;
        let mut sim = Simulation::new(config, seed);
        sim.init_default_data_centers();
        Ok(SimulationEngine { sim })
    }

    /// Generate player population
    pub fn generate_population(&mut self, count: usize) {
        self.sim.generate_population(count, None);
    }

    /// Run single tick
    pub fn tick(&mut self) {
        self.sim.tick();
    }

    /// Run multiple ticks
    pub fn run(&mut self, ticks: u64) {
        self.sim.run(ticks);
    }

    /// Get current simulation state as JSON
    pub fn get_state(&self) -> String {
        self.sim.get_state_json()
    }

    /// Get current time
    pub fn get_time(&self) -> u64 {
        self.sim.current_time
    }

    /// Get total players
    pub fn get_total_players(&self) -> usize {
        self.sim.players.len()
    }

    /// Get players by state
    pub fn get_player_counts(&self) -> String {
        serde_json::json!({
            "offline": self.sim.stats.players_offline,
            "in_lobby": self.sim.stats.players_in_lobby,
            "searching": self.sim.stats.players_searching,
            "in_match": self.sim.stats.players_in_match,
        }).to_string()
    }

    /// Get statistics JSON
    pub fn get_stats(&self) -> String {
        serde_json::to_string(&self.sim.stats).unwrap_or_default()
    }

    /// Get skill distribution as JSON
    pub fn get_skill_distribution(&self) -> String {
        serde_json::to_string(&self.sim.get_skill_distribution()).unwrap_or_default()
    }

    /// Set arrival rate
    pub fn set_arrival_rate(&mut self, rate: f64) {
        self.sim.set_arrival_rate(rate);
    }

    /// Update matchmaking config
    pub fn update_config(&mut self, config_json: &str) -> Result<(), JsValue> {
        let config: MatchmakingConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?;
        self.sim.update_config(config);
        Ok(())
    }

    /// Get default config as JSON
    pub fn get_default_config() -> String {
        serde_json::to_string(&MatchmakingConfig::default()).unwrap_or_default()
    }

    /// Get search time histogram (for visualization)
    pub fn get_search_time_histogram(&self, num_bins: usize) -> String {
        let samples = &self.sim.stats.search_time_samples;
        if samples.is_empty() {
            return "[]".to_string();
        }

        let max_time = samples.iter().cloned().fold(0.0_f64, f64::max);
        let bin_width = (max_time / num_bins as f64).max(1.0);
        
        let mut bins = vec![0usize; num_bins];
        for &sample in samples {
            let bin = ((sample / bin_width) as usize).min(num_bins - 1);
            bins[bin] += 1;
        }

        let histogram: Vec<_> = bins.iter().enumerate()
            .map(|(i, &count)| {
                serde_json::json!({
                    "bin_start": i as f64 * bin_width,
                    "bin_end": (i + 1) as f64 * bin_width,
                    "count": count,
                })
            })
            .collect();

        serde_json::to_string(&histogram).unwrap_or_default()
    }

    /// Get delta ping histogram
    pub fn get_delta_ping_histogram(&self, num_bins: usize) -> String {
        let samples = &self.sim.stats.delta_ping_samples;
        if samples.is_empty() {
            return "[]".to_string();
        }

        let max_ping = samples.iter().cloned().fold(0.0_f64, f64::max);
        let bin_width = (max_ping / num_bins as f64).max(1.0);
        
        let mut bins = vec![0usize; num_bins];
        for &sample in samples {
            let bin = ((sample / bin_width) as usize).min(num_bins - 1);
            bins[bin] += 1;
        }

        let histogram: Vec<_> = bins.iter().enumerate()
            .map(|(i, &count)| {
                serde_json::json!({
                    "bin_start": i as f64 * bin_width,
                    "bin_end": (i + 1) as f64 * bin_width,
                    "count": count,
                })
            })
            .collect();

        serde_json::to_string(&histogram).unwrap_or_default()
    }

    /// Get bucket stats as JSON
    pub fn get_bucket_stats(&self) -> String {
        serde_json::to_string(&self.sim.stats.bucket_stats).unwrap_or_default()
    }

    /// Reset statistics (keep population)
    pub fn reset_stats(&mut self) {
        self.sim.stats = SimulationStats::default();
    }

    /// Get data center info
    pub fn get_data_centers(&self) -> String {
        let dc_info: Vec<_> = self.sim.data_centers.iter()
            .map(|dc| {
                serde_json::json!({
                    "id": dc.id,
                    "name": dc.name,
                    "region": dc.region,
                    "lat": dc.location.lat,
                    "lon": dc.location.lon,
                    "busy_servers": dc.busy_servers,
                })
            })
            .collect();
        serde_json::to_string(&dc_info).unwrap_or_default()
    }

    /// Create a party from player IDs
    pub fn create_party(&mut self, player_ids_json: &str) -> Result<String, JsValue> {
        let player_ids: Vec<usize> = serde_json::from_str(player_ids_json)
            .map_err(|e| JsValue::from_str(&format!("Player IDs parse error: {}", e)))?;
        
        match self.sim.create_party(player_ids) {
            Ok(party_id) => Ok(serde_json::json!({ "party_id": party_id }).to_string()),
            Err(e) => Err(JsValue::from_str(&e)),
        }
    }

    /// Join a player to a party
    pub fn join_party(&mut self, party_id: usize, player_id: usize) -> Result<(), JsValue> {
        self.sim.join_party(party_id, player_id)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Leave a party
    pub fn leave_party(&mut self, party_id: usize, player_id: usize) -> Result<(), JsValue> {
        self.sim.leave_party(party_id, player_id)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Disband a party
    pub fn disband_party(&mut self, party_id: usize) -> Result<(), JsValue> {
        self.sim.disband_party(party_id)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Get all parties as JSON
    pub fn get_parties(&self) -> String {
        let parties: Vec<_> = self.sim.parties.iter()
            .map(|(id, party)| {
                serde_json::json!({
                    "id": id,
                    "player_ids": party.player_ids,
                    "leader_id": party.leader_id,
                    "size": party.size(),
                    "avg_skill": party.avg_skill,
                    "avg_skill_percentile": party.avg_skill_percentile,
                    "skill_disparity": party.skill_disparity,
                })
            })
            .collect();
        serde_json::to_string(&parties).unwrap_or_default()
    }

    /// Get party members
    pub fn get_party_members(&self, party_id: usize) -> String {
        let members = self.sim.get_party_members(party_id);
        serde_json::to_string(&members).unwrap_or_default()
    }

    /// Get players in lobby (for party creation UI)
    pub fn get_lobby_players(&self) -> String {
        let lobby_players: Vec<_> = self.sim.players.iter()
            .filter(|(_, p)| p.state == PlayerState::InLobby)
            .map(|(id, p)| {
                serde_json::json!({
                    "id": id,
                    "skill": p.skill,
                    "skill_percentile": p.skill_percentile,
                    "party_id": p.party_id,
                })
            })
            .collect();
        serde_json::to_string(&lobby_players).unwrap_or_default()
    }

    /// Get active search objects (for search queue visualization)
    pub fn get_search_queue(&self) -> String {
        let searches: Vec<_> = self.sim.searches.iter()
            .map(|s| {
                // Check if this is a party search by checking if any player has a party_id
                let is_party = s.player_ids.iter().any(|&pid| {
                    self.sim.players.get(&pid)
                        .map(|p| p.party_id.is_some())
                        .unwrap_or(false)
                });
                
                serde_json::json!({
                    "id": s.id,
                    "player_ids": s.player_ids,
                    "size": s.size(),
                    "is_party": is_party,
                    "avg_skill_percentile": s.avg_skill_percentile,
                    "wait_time": s.wait_time(self.sim.current_time, self.sim.config.tick_interval),
                })
            })
            .collect();
        serde_json::to_string(&searches).unwrap_or_default()
    }

    /// Get skill evolution data (time series of skill distribution)
    pub fn get_skill_evolution_data(&self) -> String {
        let evolution_data: Vec<_> = self.sim.stats.skill_distribution_over_time.iter()
            .map(|(tick, buckets)| {
                serde_json::json!({
                    "tick": tick,
                    "buckets": buckets.iter().map(|(bucket_id, mean_skill)| {
                        serde_json::json!({
                            "bucket": bucket_id,
                            "mean_skill": mean_skill,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::to_string(&evolution_data).unwrap_or_default()
    }

    /// Get performance distribution histogram
    pub fn get_performance_distribution(&self, num_bins: usize) -> String {
        let samples = &self.sim.stats.performance_samples;
        if samples.is_empty() {
            return "[]".to_string();
        }

        let max_perf = samples.iter().cloned().fold(0.0_f64, f64::max);
        let min_perf = samples.iter().cloned().fold(1.0_f64, f64::min);
        let bin_width = ((max_perf - min_perf) / num_bins as f64).max(0.01);
        
        let mut bins = vec![0usize; num_bins];
        for &sample in samples {
            let bin = (((sample - min_perf) / bin_width) as usize).min(num_bins - 1);
            bins[bin] += 1;
        }

        let histogram: Vec<_> = bins.iter().enumerate()
            .map(|(i, &count)| {
                serde_json::json!({
                    "bin_start": min_perf + i as f64 * bin_width,
                    "bin_end": min_perf + (i + 1) as f64 * bin_width,
                    "count": count,
                })
            })
            .collect();

        serde_json::to_string(&histogram).unwrap_or_default()
    }

    /// Toggle skill evolution mode (static vs evolving)
    pub fn toggle_skill_evolution(&mut self, enabled: bool) {
        self.sim.config.enable_skill_evolution = enabled;
    }

    /// Get retention statistics as JSON
    pub fn get_retention_stats(&self) -> String {
        serde_json::json!({
            "per_bucket_continue_rate": self.sim.stats.per_bucket_continue_rate,
            "avg_computed_continue_prob": self.sim.stats.avg_computed_continue_prob,
            "sample_logits": self.sim.stats.sample_logits.clone(),
            "sample_experiences": self.sim.stats.sample_experiences.clone(),
            "current_retention_config": self.sim.stats.current_retention_config.clone(),
            "avg_matches_per_session": self.sim.stats.avg_matches_per_session,
            "active_sessions": self.sim.stats.active_sessions,
            "total_sessions_completed": self.sim.stats.total_sessions_completed,
            "churn_rate": self.sim.stats.churn_rate,
            "players_leaving_rate": self.sim.stats.players_leaving_rate,
            "population_change_rate": self.sim.stats.population_change_rate,
            "recent_population_samples": self.sim.stats.recent_population_samples.clone(),
            "per_bucket_return_rate": self.sim.stats.per_bucket_return_rate,
            "total_return_attempts": self.sim.stats.total_return_attempts,
            "total_returns": self.sim.stats.total_returns,
        }).to_string()
    }

    /// Get churn rate
    pub fn get_churn_rate(&self) -> f64 {
        self.sim.stats.churn_rate
    }

    /// Get effective population size history (time series)
    pub fn get_effective_population_history(&self) -> String {
        let history: Vec<_> = self.sim.stats.effective_population_size_over_time.iter()
            .map(|(tick, size)| {
                serde_json::json!({
                    "tick": tick,
                    "population": size,
                })
            })
            .collect();
        serde_json::to_string(&history).unwrap_or_default()
    }

    /// Get return statistics as JSON
    pub fn get_return_stats(&self) -> String {
        serde_json::json!({
            "per_bucket_return_rate": self.sim.stats.per_bucket_return_rate,
            "churn_rate": self.sim.stats.churn_rate,
            "total_return_attempts": self.sim.stats.total_return_attempts,
            "total_returns": self.sim.stats.total_returns,
            "churn_threshold_ticks": self.sim.stats.churn_threshold_ticks,
        }).to_string()
    }

    /// Get session statistics as JSON
    pub fn get_session_stats(&self) -> String {
        serde_json::json!({
            "session_length_distribution": self.sim.stats.session_length_distribution,
            "avg_matches_per_session": self.sim.stats.avg_matches_per_session,
            "active_sessions": self.sim.stats.active_sessions,
            "total_sessions_completed": self.sim.stats.total_sessions_completed,
        }).to_string()
    }
}

/// Run a parameter sweep experiment
#[wasm_bindgen]
pub fn run_experiment(
    base_config_json: &str,
    parameter: &str,
    values_json: &str,
    population: usize,
    ticks_per_run: u64,
    seed: u64,
) -> Result<String, JsValue> {
    let base_config: MatchmakingConfig = serde_json::from_str(base_config_json)
        .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?;
    
    let values: Vec<f64> = serde_json::from_str(values_json)
        .map_err(|e| JsValue::from_str(&format!("Values parse error: {}", e)))?;

    let mut results = Vec::new();

    for (i, &value) in values.iter().enumerate() {
        let mut config = base_config.clone();
        
        // Update the specified parameter
        match parameter {
            "skill_similarity_initial" => config.skill_similarity_initial = value,
            "skill_similarity_rate" => config.skill_similarity_rate = value,
            "skill_similarity_max" => config.skill_similarity_max = value,
            "max_skill_disparity_initial" => config.max_skill_disparity_initial = value,
            "max_skill_disparity_rate" => config.max_skill_disparity_rate = value,
            "delta_ping_initial" => config.delta_ping_initial = value,
            "delta_ping_rate" => config.delta_ping_rate = value,
            "weight_skill" => config.weight_skill = value,
            "weight_geo" => config.weight_geo = value,
            _ => {
                return Err(JsValue::from_str(&format!("Unknown parameter: {}", parameter)));
            }
        }

        let mut sim = Simulation::new(config, seed + i as u64);
        sim.init_default_data_centers();
        sim.generate_population(population, None);
        sim.run(ticks_per_run);

        results.push(serde_json::json!({
            "parameter_value": value,
            "avg_search_time": sim.stats.avg_search_time,
            "search_time_p90": sim.stats.search_time_p90,
            "avg_delta_ping": sim.stats.avg_delta_ping,
            "delta_ping_p90": sim.stats.delta_ping_p90,
            "avg_skill_disparity": sim.stats.avg_skill_disparity,
            "blowout_rate": sim.stats.blowout_rate,
            "total_matches": sim.stats.total_matches,
        }));
    }

    serde_json::to_string(&results)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// Compare two configs
#[wasm_bindgen]
pub fn compare_configs(
    config_a_json: &str,
    config_b_json: &str,
    population: usize,
    ticks: u64,
    seed: u64,
) -> Result<String, JsValue> {
    let config_a: MatchmakingConfig = serde_json::from_str(config_a_json)
        .map_err(|e| JsValue::from_str(&format!("Config A parse error: {}", e)))?;
    let config_b: MatchmakingConfig = serde_json::from_str(config_b_json)
        .map_err(|e| JsValue::from_str(&format!("Config B parse error: {}", e)))?;

    // Run simulation A
    let mut sim_a = Simulation::new(config_a, seed);
    sim_a.init_default_data_centers();
    sim_a.generate_population(population, None);
    sim_a.run(ticks);

    // Run simulation B
    let mut sim_b = Simulation::new(config_b, seed);
    sim_b.init_default_data_centers();
    sim_b.generate_population(population, None);
    sim_b.run(ticks);

    let comparison = serde_json::json!({
        "config_a": {
            "stats": sim_a.stats,
            "bucket_stats": sim_a.stats.bucket_stats,
        },
        "config_b": {
            "stats": sim_b.stats,
            "bucket_stats": sim_b.stats.bucket_stats,
        }
    });

    serde_json::to_string(&comparison)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}
