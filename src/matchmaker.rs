use crate::types::*;
use rand::Rng;
use std::collections::{HashMap, HashSet};

/// The matchmaking engine
pub struct Matchmaker {
    config: MatchmakingConfig,
}

impl Matchmaker {
    pub fn new(config: MatchmakingConfig) -> Self {
        Self { config }
    }

    /// Calculate distance between two search objects
    pub fn calculate_distance(
        &self,
        search_a: &SearchObject,
        search_b: &SearchObject,
    ) -> f64 {
        // Geographic distance (normalized to 0-1 scale, max ~20000km)
        let geo_dist = search_a.avg_location.distance_km(&search_b.avg_location) / 20000.0;
        
        // Skill distance
        let skill_dist = (search_a.avg_skill_percentile - search_b.avg_skill_percentile).abs();
        
        // Input device penalty (0 if same, 1 if different mix)
        let input_dist = self.input_device_distance(search_a, search_b);
        
        // Platform penalty
        let platform_dist = self.platform_distance(search_a, search_b);
        
        self.config.weight_geo * geo_dist
            + self.config.weight_skill * skill_dist
            + self.config.weight_input * input_dist
            + self.config.weight_platform * platform_dist
    }

    fn input_device_distance(&self, a: &SearchObject, b: &SearchObject) -> f64 {
        let a_mkb = a.input_devices.get(&InputDevice::MouseKeyboard).copied().unwrap_or(0);
        let b_mkb = b.input_devices.get(&InputDevice::MouseKeyboard).copied().unwrap_or(0);
        let a_ctrl = a.input_devices.get(&InputDevice::Controller).copied().unwrap_or(0);
        let b_ctrl = b.input_devices.get(&InputDevice::Controller).copied().unwrap_or(0);
        
        // Penalty if mixing input devices
        if (a_mkb > 0 && b_ctrl > 0) || (a_ctrl > 0 && b_mkb > 0) {
            0.5
        } else {
            0.0
        }
    }

    fn platform_distance(&self, a: &SearchObject, b: &SearchObject) -> f64 {
        // Check if platforms overlap
        let a_platforms: HashSet<_> = a.platforms.keys().collect();
        let b_platforms: HashSet<_> = b.platforms.keys().collect();
        
        if a_platforms.is_disjoint(&b_platforms) {
            0.3 // Cross-platform penalty
        } else {
            0.0
        }
    }

    /// Check if two search objects can be combined into a lobby
    pub fn check_feasibility(
        &self,
        searches: &[&SearchObject],
        playlist: Playlist,
        current_time: u64,
        data_centers: &[DataCenter],
    ) -> Option<FeasibilityResult> {
        // 1. Check playlist compatibility
        for search in searches {
            if !search.acceptable_playlists.contains(&playlist) {
                return None;
            }
        }

        // 2. Check total size
        let total_size: usize = searches.iter().map(|s| s.size()).sum();
        if total_size > playlist.required_players() {
            return None;
        }

        // 3. Check skill similarity
        let min_skill = searches.iter().map(|s| s.avg_skill_percentile).fold(f64::MAX, f64::min);
        let max_skill = searches.iter().map(|s| s.avg_skill_percentile).fold(f64::MIN, f64::max);
        let skill_range = max_skill - min_skill;
        
        for search in searches {
            let wait_time = search.wait_time(current_time);
            let allowed_range = self.config.skill_similarity_backoff(wait_time);
            
            // Check if this search can accept the skill range
            if skill_range > allowed_range * 2.0 {
                return None;
            }
        }

        // 4. Check skill disparity
        let max_disparity_allowed = searches
            .iter()
            .map(|s| {
                let wait_time = s.wait_time(current_time);
                self.config.skill_disparity_backoff(wait_time)
            })
            .fold(f64::MAX, f64::min);
        
        if skill_range > max_disparity_allowed {
            return None;
        }

        // 5. Find common acceptable data centers
        let common_dcs: HashSet<usize> = searches
            .iter()
            .map(|s| &s.acceptable_dcs)
            .fold(None::<HashSet<usize>>, |acc, dcs| {
                Some(match acc {
                    None => dcs.clone(),
                    Some(common) => common.intersection(dcs).copied().collect(),
                })
            })
            .unwrap_or_default();

        if common_dcs.is_empty() {
            return None;
        }

        // 6. Check server capacity - find a DC with available server
        let available_dc = common_dcs.iter().find(|&&dc_id| {
            data_centers.iter()
                .find(|dc| dc.id == dc_id)
                .map(|dc| dc.available_servers(&playlist) > 0)
                .unwrap_or(false)
        });

        available_dc.map(|&dc_id| FeasibilityResult {
            data_center_id: dc_id,
            skill_disparity: skill_range,
        })
    }

    /// Calculate quality score for a potential match
    pub fn calculate_quality(
        &self,
        searches: &[&SearchObject],
        players: &HashMap<usize, Player>,
        dc_id: usize,
        current_time: u64,
    ) -> f64 {
        // Ping quality (lower delta ping = higher quality)
        let mut total_delta_ping = 0.0;
        let mut player_count = 0;
        
        for search in searches {
            for &player_id in &search.player_ids {
                if let Some(player) = players.get(&player_id) {
                    if let Some(&ping) = player.dc_pings.get(&dc_id) {
                        total_delta_ping += ping - player.best_ping;
                        player_count += 1;
                    }
                }
            }
        }
        
        let avg_delta_ping = if player_count > 0 {
            total_delta_ping / player_count as f64
        } else {
            0.0
        };
        let ping_quality = 1.0 - (avg_delta_ping / self.config.max_ping).min(1.0);

        // Skill balance (how close are the teams likely to be)
        let skills: Vec<f64> = searches.iter()
            .map(|s| s.avg_skill_percentile)
            .collect();
        let skill_variance = if skills.len() > 1 {
            let mean = skills.iter().sum::<f64>() / skills.len() as f64;
            skills.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / skills.len() as f64
        } else {
            0.0
        };
        let skill_balance_quality = 1.0 - (skill_variance * 4.0).min(1.0);

        // Wait time fairness (reward matching players who've waited longer)
        let avg_wait = searches.iter()
            .map(|s| s.wait_time(current_time))
            .sum::<f64>() / searches.len() as f64;
        let wait_quality = (avg_wait / 60.0).min(1.0); // Bonus for reducing long waits

        self.config.quality_weight_ping * ping_quality
            + self.config.quality_weight_skill_balance * skill_balance_quality
            + self.config.quality_weight_wait_time * wait_quality
    }

    /// Run one matchmaking tick
    pub fn run_tick(
        &self,
        searches: &mut Vec<SearchObject>,
        players: &mut HashMap<usize, Player>,
        data_centers: &mut [DataCenter],
        parties: &HashMap<usize, Party>,
        current_time: u64,
        rng: &mut impl Rng,
    ) -> Vec<MatchResult> {
        let mut results = Vec::new();
        let mut matched_search_ids: HashSet<usize> = HashSet::new();

        // Update acceptable DCs for all searches based on current wait time
        for search in searches.iter_mut() {
            let wait_time = search.wait_time(current_time);
            let mut acceptable = HashSet::new();
            
            for &player_id in &search.player_ids {
                if let Some(player) = players.get(&player_id) {
                    let player_dcs: HashSet<_> = player
                        .acceptable_dcs(wait_time, &self.config)
                        .into_iter()
                        .collect();
                    
                    if acceptable.is_empty() {
                        acceptable = player_dcs;
                    } else {
                        acceptable = acceptable.intersection(&player_dcs).copied().collect();
                    }
                }
            }
            search.acceptable_dcs = acceptable;
        }

        // Sort searches by wait time (longest waiting = highest priority as seeds)
        let mut search_order: Vec<usize> = (0..searches.len()).collect();
        search_order.sort_by(|&a, &b| {
            let wait_a = searches[a].wait_time(current_time);
            let wait_b = searches[b].wait_time(current_time);
            wait_b.partial_cmp(&wait_a).unwrap()
        });

        // Process each playlist separately
        for playlist in [
            Playlist::TeamDeathmatch,
            Playlist::SearchAndDestroy,
            Playlist::Domination,
            Playlist::GroundWar,
            Playlist::FreeForAll,
        ] {
            let required_size = playlist.required_players();

            // Get searches for this playlist
            let playlist_searches: Vec<usize> = search_order
                .iter()
                .copied()
                .filter(|&idx| {
                    !matched_search_ids.contains(&searches[idx].id)
                        && searches[idx].acceptable_playlists.contains(&playlist)
                })
                .collect();

            if playlist_searches.is_empty() {
                continue;
            }

            // Use each unmatched search as a potential seed
            for &seed_idx in &playlist_searches {
                if matched_search_ids.contains(&searches[seed_idx].id) {
                    continue;
                }

                let seed = &searches[seed_idx];
                
                // Find candidates (sorted by distance from seed)
                let mut candidates: Vec<(usize, f64)> = playlist_searches
                    .iter()
                    .filter(|&&idx| {
                        idx != seed_idx && !matched_search_ids.contains(&searches[idx].id)
                    })
                    .map(|&idx| {
                        let dist = self.calculate_distance(seed, &searches[idx]);
                        (idx, dist)
                    })
                    .collect();
                
                candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                candidates.truncate(self.config.top_k_candidates);

                // Greedy lobby construction
                let mut lobby_indices = vec![seed_idx];
                let mut lobby_size = seed.size();

                for (cand_idx, _) in candidates {
                    if lobby_size >= required_size {
                        break;
                    }

                    let candidate = &searches[cand_idx];
                    if lobby_size + candidate.size() > required_size {
                        continue;
                    }

                    // Check feasibility
                    let lobby_searches: Vec<_> = lobby_indices
                        .iter()
                        .map(|&i| &searches[i])
                        .chain(std::iter::once(candidate))
                        .collect();

                    if self.check_feasibility(&lobby_searches, playlist, current_time, data_centers).is_some() {
                        lobby_indices.push(cand_idx);
                        lobby_size += candidate.size();
                    }
                }

                // If we have a full lobby, create the match
                if lobby_size == required_size {
                    let lobby_searches: Vec<_> = lobby_indices
                        .iter()
                        .map(|&i| &searches[i])
                        .collect();

                    if let Some(feasibility) = self.check_feasibility(
                        &lobby_searches,
                        playlist,
                        current_time,
                        data_centers,
                    ) {
                        let quality = self.calculate_quality(
                            &lobby_searches,
                            players,
                            feasibility.data_center_id,
                            current_time,
                        );

                        // Collect all player IDs
                        let all_players: Vec<usize> = lobby_searches
                            .iter()
                            .flat_map(|s| s.player_ids.iter().copied())
                            .collect();

                        // Calculate average delta ping
                        let avg_delta_ping = all_players
                            .iter()
                            .filter_map(|&pid| {
                                players.get(&pid).and_then(|p| {
                                    p.dc_pings.get(&feasibility.data_center_id)
                                        .map(|ping| ping - p.best_ping)
                                })
                            })
                            .sum::<f64>() / all_players.len() as f64;

                        // Calculate search times
                        let search_times: Vec<f64> = lobby_searches
                            .iter()
                            .map(|s| s.wait_time(current_time) * self.config.tick_interval)
                            .collect();

                        // Create teams using skill-based balancing
                        let teams = self.balance_teams(&all_players, players, parties, playlist, rng);

                        // Mark searches as matched
                        for &idx in &lobby_indices {
                            matched_search_ids.insert(searches[idx].id);
                        }

                        // Reserve server
                        if let Some(dc) = data_centers.iter_mut().find(|dc| dc.id == feasibility.data_center_id) {
                            if let Some(busy) = dc.busy_servers.get_mut(&playlist) {
                                *busy += 1;
                            }
                        }

                        results.push(MatchResult {
                            player_ids: all_players,
                            teams,
                            playlist,
                            data_center_id: feasibility.data_center_id,
                            quality_score: quality,
                            skill_disparity: feasibility.skill_disparity,
                            avg_delta_ping,
                            search_times,
                        });
                    }
                }
            }
        }

        // Remove matched searches
        searches.retain(|s| !matched_search_ids.contains(&s.id));

        results
    }

    /// Balance teams based on skill, respecting party boundaries
    fn balance_teams(
        &self,
        player_ids: &[usize],
        players: &HashMap<usize, Player>,
        parties: &HashMap<usize, Party>,
        playlist: Playlist,
        rng: &mut impl Rng,
    ) -> Vec<Vec<usize>> {
        let team_count = playlist.team_count();
        
        if team_count == player_ids.len() {
            // FFA - each player is their own team
            return player_ids.iter().map(|&id| vec![id]).collect();
        }

        // Group players by party_id (solo players are their own "party")
        let mut party_groups: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
        for &player_id in player_ids {
            let party_id = players.get(&player_id).and_then(|p| p.party_id);
            party_groups.entry(party_id).or_insert_with(Vec::new).push(player_id);
        }

        // Compute party aggregates and create party entries for balancing
        let mut party_entries: Vec<(Option<usize>, Vec<usize>, f64)> = Vec::new();
        for (party_id, member_ids) in party_groups {
            let avg_skill = if let Some(pid) = party_id {
                // Get avg_skill from party
                parties.get(&pid)
                    .map(|p| p.avg_skill)
                    .unwrap_or_else(|| {
                        // Fallback: compute from members
                        member_ids.iter()
                            .filter_map(|id| players.get(id).map(|p| p.skill))
                            .sum::<f64>() / member_ids.len() as f64
                    })
            } else {
                // Solo player: use individual skill
                member_ids.first()
                    .and_then(|id| players.get(id).map(|p| p.skill))
                    .unwrap_or(0.0)
            };
            party_entries.push((party_id, member_ids, avg_skill));
        }

        // Sort parties by avg_skill (descending)
        party_entries.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        // Snake draft: assign entire parties to teams
        let mut teams: Vec<Vec<usize>> = vec![Vec::new(); team_count];
        let mut forward = true;
        let mut team_idx = 0;

        for (_, member_ids, _) in party_entries {
            // Assign all members of this party to the same team
            for &player_id in &member_ids {
                teams[team_idx].push(player_id);
            }
            
            // Move to next team in snake draft
            if forward {
                if team_idx == team_count - 1 {
                    forward = false;
                } else {
                    team_idx += 1;
                }
            } else {
                if team_idx == 0 {
                    forward = true;
                } else {
                    team_idx -= 1;
                }
            }
        }

        // Verify team sizes are balanced (allow small differences due to party sizes)
        // No randomization needed since party integrity is maintained

        teams
    }
}

#[derive(Debug)]
pub struct FeasibilityResult {
    pub data_center_id: usize,
    pub skill_disparity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub player_ids: Vec<usize>,
    pub teams: Vec<Vec<usize>>,
    pub playlist: Playlist,
    pub data_center_id: usize,
    pub quality_score: f64,
    pub skill_disparity: f64,
    pub avg_delta_ping: f64,
    pub search_times: Vec<f64>,
}

use serde::{Deserialize, Serialize};
