import React, { useState, useEffect, useCallback, useRef } from 'react';
import { LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, ScatterChart, Scatter, Cell, RadarChart, PolarGrid, PolarAngleAxis, PolarRadiusAxis, Radar } from 'recharts';

// ============================================================================
// SIMULATION ENGINE (JavaScript implementation mirroring the Rust code)
// ============================================================================

class Location {
  constructor(lat, lon) {
    this.lat = lat;
    this.lon = lon;
  }
  
  distanceKm(other) {
    const R = 6371;
    const dLat = (other.lat - this.lat) * Math.PI / 180;
    const dLon = (other.lon - this.lon) * Math.PI / 180;
    const lat1 = this.lat * Math.PI / 180;
    const lat2 = other.lat * Math.PI / 180;
    const a = Math.sin(dLat/2)**2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLon/2)**2;
    return R * 2 * Math.asin(Math.sqrt(a));
  }
}

const PLAYLISTS = {
  TeamDeathmatch: { name: 'TDM', required: 12, duration: 600 },
  SearchAndDestroy: { name: 'S&D', required: 12, duration: 900 },
  Domination: { name: 'DOM', required: 12, duration: 600 },
  GroundWar: { name: 'GW', required: 64, duration: 1200 },
  FreeForAll: { name: 'FFA', required: 12, duration: 600 },
};

const DATA_CENTERS = [
  { id: 0, name: 'US-East', location: new Location(39, -77), region: 'NA' },
  { id: 1, name: 'US-West', location: new Location(37, -122), region: 'NA' },
  { id: 2, name: 'US-Central', location: new Location(41, -96), region: 'NA' },
  { id: 3, name: 'EU-West', location: new Location(51, 0), region: 'EU' },
  { id: 4, name: 'EU-Central', location: new Location(50, 8), region: 'EU' },
  { id: 5, name: 'EU-North', location: new Location(59, 18), region: 'EU' },
  { id: 6, name: 'Asia-East', location: new Location(35, 139), region: 'APAC' },
  { id: 7, name: 'Asia-SE', location: new Location(1, 103), region: 'APAC' },
  { id: 8, name: 'Australia', location: new Location(-33, 151), region: 'APAC' },
  { id: 9, name: 'South-America', location: new Location(-23, -46), region: 'SA' },
];

const defaultConfig = {
  maxPing: 200,
  deltaPingInitial: 10,
  deltaPingRate: 2,
  deltaPingMax: 100,
  skillSimilarityInitial: 0.05,
  skillSimilarityRate: 0.01,
  skillSimilarityMax: 0.5,
  maxSkillDisparityInitial: 0.1,
  maxSkillDisparityRate: 0.02,
  maxSkillDisparityMax: 0.8,
  weightGeo: 0.3,
  weightSkill: 0.4,
  weightInput: 0.15,
  weightPlatform: 0.15,
  tickInterval: 5,
  numSkillBuckets: 10,
  topKCandidates: 50,
  arrivalRate: 10,
};

class SimulationEngine {
  constructor(config = defaultConfig, seed = 12345) {
    this.config = { ...defaultConfig, ...config };
    this.seed = seed;
    this.rng = this.createRng(seed);
    this.reset();
  }

  createRng(seed) {
    let s = seed;
    return () => {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      return s / 0x7fffffff;
    };
  }

  reset() {
    this.currentTime = 0;
    this.players = new Map();
    this.searches = [];
    this.matches = new Map();
    this.nextPlayerId = 0;
    this.nextSearchId = 0;
    this.nextMatchId = 0;
    this.dcServers = DATA_CENTERS.map(dc => ({ ...dc, busy: {} }));
    Object.keys(PLAYLISTS).forEach(p => this.dcServers.forEach(dc => dc.busy[p] = 0));
    
    this.stats = {
      totalMatches: 0,
      searchTimeSamples: [],
      deltaPingSamples: [],
      skillDisparitySamples: [],
      blowoutCount: 0,
      timeSeriesData: [],
    };
    
    this.rng = this.createRng(this.seed);
  }

  generatePopulation(count) {
    // Clear existing players first
    this.players.clear();
    this.nextPlayerId = 0;
    
    console.log(`Generating population of ${count} players...`);
    
    const regions = [
      { loc: new Location(39, -95), weight: 0.35 },
      { loc: new Location(50, 10), weight: 0.30 },
      { loc: new Location(35, 105), weight: 0.20 },
      { loc: new Location(-25, 135), weight: 0.08 },
      { loc: new Location(-15, -55), weight: 0.07 },
    ];

    for (let i = 0; i < count; i++) {
      const r = this.rng();
      let cumulative = 0;
      let regionLoc = regions[0].loc;
      for (const reg of regions) {
        cumulative += reg.weight;
        if (r < cumulative) {
          regionLoc = reg.loc;
          break;
        }
      }

      const location = new Location(
        regionLoc.lat + (this.rng() - 0.5) * 20,
        regionLoc.lon + (this.rng() - 0.5) * 30
      );

      // Skill: approximate normal distribution
      let skill = 0;
      for (let j = 0; j < 12; j++) skill += this.rng();
      skill = ((skill - 6) / 3);
      skill = Math.max(-1, Math.min(1, skill));

      const player = {
        id: this.nextPlayerId++,
        location,
        platform: ['PC', 'PlayStation', 'Xbox'][Math.floor(this.rng() * 3)],
        inputDevice: this.rng() < 0.6 ? 'Controller' : 'MouseKeyboard',
        skill,
        skillPercentile: 0.5,
        skillBucket: 5,
        state: 'Offline',
        currentMatch: null,
        preferredPlaylists: ['TeamDeathmatch'],
        dcPings: {},
        bestDc: null,
        bestPing: 1000,
        searchStartTime: null,
        matchesPlayed: 0,
        wins: 0,
        losses: 0,
        recentDeltaPings: [],
        recentSearchTimes: [],
        recentBlowouts: [],
        continuationProb: 0.85,
      };

      // Add additional playlists
      if (this.rng() < 0.4) player.preferredPlaylists.push('Domination');
      if (this.rng() < 0.2) player.preferredPlaylists.push('SearchAndDestroy');

      // Calculate pings
      for (const dc of DATA_CENTERS) {
        const baseDist = location.distanceKm(dc.location);
        const basePing = baseDist / 100 + 15;
        const jitter = (this.rng() - 0.5) * 15;
        player.dcPings[dc.id] = Math.max(10, basePing + jitter);
      }

      let bestPing = Infinity, bestDc = null;
      for (const [dcId, ping] of Object.entries(player.dcPings)) {
        if (ping < bestPing) {
          bestPing = ping;
          bestDc = parseInt(dcId);
        }
      }
      player.bestPing = bestPing;
      player.bestDc = bestDc;

      this.players.set(player.id, player);
    }

    this.updateSkillPercentiles();
    console.log(`Population generated: ${this.players.size} players`);
  }

  updateSkillPercentiles() {
    const sorted = [...this.players.values()].sort((a, b) => a.skill - b.skill);
    const n = sorted.length;
    sorted.forEach((p, i) => {
      p.skillPercentile = (i + 0.5) / n;
      p.skillBucket = Math.floor(p.skillPercentile * this.config.numSkillBuckets) + 1;
      p.skillBucket = Math.max(1, Math.min(this.config.numSkillBuckets, p.skillBucket));
    });
  }

  deltaPingBackoff(waitTime) {
    return Math.min(
      this.config.deltaPingInitial + this.config.deltaPingRate * waitTime,
      this.config.deltaPingMax
    );
  }

  skillSimilarityBackoff(waitTime) {
    return Math.min(
      this.config.skillSimilarityInitial + this.config.skillSimilarityRate * waitTime,
      this.config.skillSimilarityMax
    );
  }

  tick() {
    // 1. Arrivals
    const offlinePlayers = [...this.players.values()].filter(p => p.state === 'Offline');
    const numArrivals = this.poissonSample(this.config.arrivalRate);
    for (let i = 0; i < Math.min(numArrivals, offlinePlayers.length); i++) {
      const idx = Math.floor(this.rng() * offlinePlayers.length);
      const player = offlinePlayers.splice(idx, 1)[0];
      player.state = 'InLobby';
    }

    // 2. Search starts
    const lobbyPlayers = [...this.players.values()].filter(p => p.state === 'InLobby');
    for (const player of lobbyPlayers) {
      if (this.rng() < 0.3) {
        this.startSearch(player);
      }
    }

    // 3. Matchmaking
    this.runMatchmaking();

    // 4. Match completions
    this.processMatchCompletions();

    // 5. Update stats
    this.updateStats();

    this.currentTime++;
  }

  startSearch(player) {
    player.state = 'Searching';
    player.searchStartTime = this.currentTime;

    const search = {
      id: this.nextSearchId++,
      playerIds: [player.id],
      avgSkillPercentile: player.skillPercentile,
      avgLocation: player.location,
      acceptablePlaylists: [...player.preferredPlaylists],
      searchStartTime: this.currentTime,
      acceptableDcs: new Set(Object.keys(player.dcPings).map(Number)),
    };

    this.searches.push(search);
  }

  runMatchmaking() {
    const matched = new Set();

    // Update acceptable DCs
    for (const search of this.searches) {
      const waitTime = (this.currentTime - search.searchStartTime) * this.config.tickInterval;
      const deltaPingAllowed = this.deltaPingBackoff(waitTime);
      
      const newDcs = new Set();
      for (const playerId of search.playerIds) {
        const player = this.players.get(playerId);
        if (!player) continue;
        
        for (const [dcId, ping] of Object.entries(player.dcPings)) {
          if (ping <= player.bestPing + deltaPingAllowed && ping <= this.config.maxPing) {
            newDcs.add(parseInt(dcId));
          }
        }
      }
      search.acceptableDcs = newDcs;
    }

    // Sort by wait time (longest first)
    const sortedSearches = [...this.searches].sort((a, b) => a.searchStartTime - b.searchStartTime);

    // Process each playlist
    for (const playlistName of Object.keys(PLAYLISTS)) {
      const playlist = PLAYLISTS[playlistName];
      const playlistSearches = sortedSearches.filter(s => 
        !matched.has(s.id) && s.acceptablePlaylists.includes(playlistName)
      );

      for (const seed of playlistSearches) {
        if (matched.has(seed.id)) continue;

        // Find candidates
        const candidates = playlistSearches
          .filter(s => s.id !== seed.id && !matched.has(s.id))
          .map(s => ({ search: s, dist: this.calculateDistance(seed, s) }))
          .sort((a, b) => a.dist - b.dist)
          .slice(0, this.config.topKCandidates);

        // Greedy construction
        const lobby = [seed];
        let lobbySize = seed.playerIds.length;

        for (const { search: cand } of candidates) {
          if (lobbySize >= playlist.required) break;
          if (lobbySize + cand.playerIds.length > playlist.required) continue;

          if (this.checkFeasibility([...lobby, cand], playlistName)) {
            lobby.push(cand);
            lobbySize += cand.playerIds.length;
          }
        }

        // Create match if full
        if (lobbySize === playlist.required) {
          const allPlayerIds = lobby.flatMap(s => s.playerIds);
          
          // Find common DC
          let commonDcs = new Set(lobby[0].acceptableDcs);
          for (const s of lobby.slice(1)) {
            commonDcs = new Set([...commonDcs].filter(dc => s.acceptableDcs.has(dc)));
          }
          
          const dcId = [...commonDcs][0];
          if (dcId === undefined) continue;

          // Calculate stats
          const searchTimes = lobby.map(s => 
            (this.currentTime - s.searchStartTime) * this.config.tickInterval
          );
          
          let totalDeltaPing = 0;
          for (const pid of allPlayerIds) {
            const p = this.players.get(pid);
            if (p) totalDeltaPing += (p.dcPings[dcId] || 0) - p.bestPing;
          }
          const avgDeltaPing = totalDeltaPing / allPlayerIds.length;

          const skills = lobby.map(s => s.avgSkillPercentile);
          const skillDisparity = Math.max(...skills) - Math.min(...skills);

          // Create teams
          const teams = this.balanceTeams(allPlayerIds, playlistName);
          
          const teamSkills = teams.map(team => {
            const teamSkill = team.reduce((sum, pid) => {
              const p = this.players.get(pid);
              return sum + (p ? p.skill : 0);
            }, 0) / team.length;
            return teamSkill;
          });

          const match = {
            id: this.nextMatchId++,
            playlist: playlistName,
            dcId,
            teams,
            teamSkills,
            startTime: this.currentTime,
            duration: Math.floor(playlist.duration * (0.8 + this.rng() * 0.4) / this.config.tickInterval),
            skillDisparity,
            avgDeltaPing,
          };

          // Update players
          for (const pid of allPlayerIds) {
            const p = this.players.get(pid);
            if (p) {
              const searchTime = (this.currentTime - (p.searchStartTime || this.currentTime)) * this.config.tickInterval;
              p.recentSearchTimes.push(searchTime);
              if (p.recentSearchTimes.length > 10) p.recentSearchTimes.shift();
              this.stats.searchTimeSamples.push(searchTime);

              const deltaPing = (p.dcPings[dcId] || 0) - p.bestPing;
              p.recentDeltaPings.push(deltaPing);
              if (p.recentDeltaPings.length > 10) p.recentDeltaPings.shift();
              this.stats.deltaPingSamples.push(deltaPing);

              p.state = 'InMatch';
              p.currentMatch = match.id;
              p.searchStartTime = null;
            }
          }

          this.stats.skillDisparitySamples.push(skillDisparity);
          this.matches.set(match.id, match);
          this.stats.totalMatches++;

          lobby.forEach(s => matched.add(s.id));
        }
      }
    }

    this.searches = this.searches.filter(s => !matched.has(s.id));
  }

  calculateDistance(a, b) {
    const geoDist = a.avgLocation.distanceKm(b.avgLocation) / 20000;
    const skillDist = Math.abs(a.avgSkillPercentile - b.avgSkillPercentile);
    return this.config.weightGeo * geoDist + this.config.weightSkill * skillDist;
  }

  checkFeasibility(searches, playlistName) {
    const skills = searches.map(s => s.avgSkillPercentile);
    const skillRange = Math.max(...skills) - Math.min(...skills);

    for (const s of searches) {
      const waitTime = (this.currentTime - s.searchStartTime) * this.config.tickInterval;
      const allowedRange = this.skillSimilarityBackoff(waitTime);
      if (skillRange > allowedRange * 2) return false;
    }

    let commonDcs = new Set(searches[0].acceptableDcs);
    for (const s of searches.slice(1)) {
      commonDcs = new Set([...commonDcs].filter(dc => s.acceptableDcs.has(dc)));
    }
    
    return commonDcs.size > 0;
  }

  balanceTeams(playerIds, playlistName) {
    if (playlistName === 'FreeForAll') {
      return playerIds.map(id => [id]);
    }

    const sorted = [...playerIds]
      .map(id => ({ id, skill: this.players.get(id)?.skill || 0 }))
      .sort((a, b) => b.skill - a.skill);

    const teams = [[], []];
    let forward = true;
    let teamIdx = 0;

    for (const { id } of sorted) {
      teams[teamIdx].push(id);
      if (forward) {
        if (teamIdx === 1) forward = false;
        else teamIdx++;
      } else {
        if (teamIdx === 0) forward = true;
        else teamIdx--;
      }
    }

    return teams;
  }

  processMatchCompletions() {
    const completed = [];
    
    for (const [matchId, match] of this.matches) {
      if (this.currentTime >= match.startTime + match.duration) {
        completed.push(matchId);

        const skillDiff = (match.teamSkills[0] || 0) - (match.teamSkills[1] || 0);
        const pTeam0Wins = 1 / (1 + Math.exp(-2 * skillDiff));
        const winningTeam = this.rng() < pTeam0Wins ? 0 : 1;
        
        // Blowout detection: consider both skill difference and win probability
        // With balanced teams, skill differences are typically small (< 0.2)
        const skillDiffAbs = Math.abs(skillDiff);
        const winProbImbalance = Math.abs(pTeam0Wins - 0.5) * 2.0; // 0 to 1 scale
        
        let blowoutProb;
        if (skillDiffAbs > 0.1) {
          // Scale from 0.1 to 0.5 skill diff maps to 0.1 to 0.7 blowout probability
          const skillComponent = Math.min((skillDiffAbs - 0.1) / 0.4, 1.0) * 0.4;
          const imbalanceComponent = winProbImbalance * 0.3;
          blowoutProb = Math.min(0.1 + skillComponent + imbalanceComponent, 0.9);
        } else if (winProbImbalance > 0.4) {
          // Even with small skill diff, high win probability imbalance suggests blowout risk
          blowoutProb = winProbImbalance * 0.5;
        } else {
          // Small skill diff and balanced win probability - still possible but less likely
          if (skillDiffAbs > 0.05) {
            blowoutProb = 0.05 + winProbImbalance * 0.1;
          } else {
            blowoutProb = 0.02;
          }
        }
        
        const isBlowout = this.rng() < blowoutProb;

        if (isBlowout) this.stats.blowoutCount++;

        for (let teamIdx = 0; teamIdx < match.teams.length; teamIdx++) {
          const won = teamIdx === winningTeam;
          for (const pid of match.teams[teamIdx]) {
            const p = this.players.get(pid);
            if (!p) continue;

            p.matchesPlayed++;
            if (won) p.wins++;
            else p.losses++;

            p.recentBlowouts.push(isBlowout);
            if (p.recentBlowouts.length > 10) p.recentBlowouts.shift();

            p.currentMatch = null;

            // Continue probability
            const avgDeltaPing = p.recentDeltaPings.length > 0 
              ? p.recentDeltaPings.reduce((a,b) => a+b, 0) / p.recentDeltaPings.length : 0;
            const pingPenalty = Math.min(avgDeltaPing / 100, 0.2);
            
            const avgSearchTime = p.recentSearchTimes.length > 0
              ? p.recentSearchTimes.reduce((a,b) => a+b, 0) / p.recentSearchTimes.length : 0;
            const searchPenalty = Math.min(avgSearchTime / 120, 0.15);
            
            const blowoutRate = p.recentBlowouts.length > 0
              ? p.recentBlowouts.filter(b => b).length / p.recentBlowouts.length : 0;
            const blowoutPenalty = blowoutRate * 0.2;

            const continueProb = Math.max(0.3, p.continuationProb - pingPenalty - searchPenalty - blowoutPenalty);

            p.state = this.rng() < continueProb ? 'InLobby' : 'Offline';
          }
        }

        // Release server
        const dc = this.dcServers.find(d => d.id === match.dcId);
        if (dc) dc.busy[match.playlist] = Math.max(0, (dc.busy[match.playlist] || 0) - 1);
      }
    }

    completed.forEach(id => this.matches.delete(id));
  }

  updateStats() {
    const playersByState = { Offline: 0, InLobby: 0, Searching: 0, InMatch: 0 };
    for (const p of this.players.values()) {
      playersByState[p.state]++;
    }

    const timePoint = {
      time: this.currentTime * this.config.tickInterval,
      searching: playersByState.Searching,
      inMatch: playersByState.InMatch,
      inLobby: playersByState.InLobby,
      activeMatches: this.matches.size,
      avgSearchTime: this.getAvgSearchTime(),
      avgDeltaPing: this.getAvgDeltaPing(),
    };

    this.stats.timeSeriesData.push(timePoint);
    if (this.stats.timeSeriesData.length > 200) {
      this.stats.timeSeriesData.shift();
    }
  }

  poissonSample(lambda) {
    const L = Math.exp(-lambda);
    let k = 0, p = 1;
    do {
      k++;
      p *= this.rng();
    } while (p > L);
    return k - 1;
  }

  getAvgSearchTime() {
    const samples = this.stats.searchTimeSamples.slice(-100);
    return samples.length > 0 ? samples.reduce((a,b) => a+b, 0) / samples.length : 0;
  }

  getAvgDeltaPing() {
    const samples = this.stats.deltaPingSamples.slice(-100);
    return samples.length > 0 ? samples.reduce((a,b) => a+b, 0) / samples.length : 0;
  }

  getPercentile(arr, p) {
    if (arr.length === 0) return 0;
    const sorted = [...arr].sort((a,b) => a - b);
    const idx = Math.floor(sorted.length * p);
    return sorted[Math.min(idx, sorted.length - 1)];
  }

  getStats() {
    const playersByState = { Offline: 0, InLobby: 0, Searching: 0, InMatch: 0 };
    for (const p of this.players.values()) {
      playersByState[p.state]++;
    }

    const searchTimes = this.stats.searchTimeSamples.slice(-1000);
    const deltaPings = this.stats.deltaPingSamples.slice(-1000);
    const skillDisparities = this.stats.skillDisparitySamples.slice(-1000);

    return {
      currentTime: this.currentTime,
      timeElapsed: this.currentTime * this.config.tickInterval,
      totalPlayers: this.players.size,
      ...playersByState,
      activeMatches: this.matches.size,
      totalMatches: this.stats.totalMatches,
      avgSearchTime: searchTimes.length > 0 ? searchTimes.reduce((a,b) => a+b, 0) / searchTimes.length : 0,
      searchTimeP50: this.getPercentile(searchTimes, 0.5),
      searchTimeP90: this.getPercentile(searchTimes, 0.9),
      avgDeltaPing: deltaPings.length > 0 ? deltaPings.reduce((a,b) => a+b, 0) / deltaPings.length : 0,
      deltaPingP90: this.getPercentile(deltaPings, 0.9),
      avgSkillDisparity: skillDisparities.length > 0 ? skillDisparities.reduce((a,b) => a+b, 0) / skillDisparities.length : 0,
      blowoutRate: this.stats.totalMatches > 0 ? this.stats.blowoutCount / this.stats.totalMatches : 0,
      timeSeriesData: this.stats.timeSeriesData,
    };
  }

  getBucketStats() {
    const buckets = {};
    for (let b = 1; b <= this.config.numSkillBuckets; b++) {
      buckets[b] = { bucket: b, players: 0, avgSearchTime: 0, avgDeltaPing: 0, winRate: 0, matches: 0 };
    }

    for (const p of this.players.values()) {
      const b = buckets[p.skillBucket];
      if (!b) continue;
      b.players++;
      if (p.recentSearchTimes.length > 0) {
        b.avgSearchTime += p.recentSearchTimes.reduce((a,c) => a+c, 0) / p.recentSearchTimes.length;
      }
      if (p.recentDeltaPings.length > 0) {
        b.avgDeltaPing += p.recentDeltaPings.reduce((a,c) => a+c, 0) / p.recentDeltaPings.length;
      }
      b.matches += p.matchesPlayed;
      if (p.matchesPlayed > 0) {
        b.winRate += p.wins / p.matchesPlayed;
      }
    }

    for (const b of Object.values(buckets)) {
      if (b.players > 0) {
        b.avgSearchTime /= b.players;
        b.avgDeltaPing /= b.players;
        b.winRate /= b.players;
      }
    }

    return Object.values(buckets);
  }

  getSkillDistribution() {
    const bins = new Array(20).fill(0);
    for (const p of this.players.values()) {
      const idx = Math.min(19, Math.floor((p.skill + 1) / 2 * 20));
      bins[idx]++;
    }
    return bins.map((count, i) => ({
      skill: ((i / 19) * 2 - 1).toFixed(2),
      count,
    }));
  }

  getSearchTimeHistogram() {
    const samples = this.stats.searchTimeSamples.slice(-500);
    if (samples.length === 0) return [];
    
    const max = Math.max(...samples);
    const binWidth = Math.max(5, max / 15);
    const bins = new Array(15).fill(0);
    
    for (const s of samples) {
      const idx = Math.min(14, Math.floor(s / binWidth));
      bins[idx]++;
    }
    
    return bins.map((count, i) => ({
      range: `${(i * binWidth).toFixed(0)}-${((i+1) * binWidth).toFixed(0)}s`,
      count,
    }));
  }

  getDeltaPingHistogram() {
    const samples = this.stats.deltaPingSamples.slice(-500);
    if (samples.length === 0) return [];
    
    const max = Math.max(...samples, 1);
    const binWidth = Math.max(5, max / 12);
    const bins = new Array(12).fill(0);
    
    for (const s of samples) {
      const idx = Math.min(11, Math.floor(s / binWidth));
      bins[idx]++;
    }
    
    return bins.map((count, i) => ({
      range: `${(i * binWidth).toFixed(0)}-${((i+1) * binWidth).toFixed(0)}ms`,
      count,
    }));
  }
}

// ============================================================================
// REACT COMPONENTS
// ============================================================================

const COLORS = {
  primary: '#00d4aa',
  secondary: '#ff6b6b',
  tertiary: '#4ecdc4',
  quaternary: '#ffe66d',
  dark: '#0a0f1c',
  darker: '#060912',
  card: '#111827',
  cardHover: '#1f2937',
  border: '#1e3a5f',
  text: '#e2e8f0',
  textMuted: '#94a3b8',
  success: '#10b981',
  warning: '#f59e0b',
  danger: '#ef4444',
};

export default function MatchmakingSimulator() {
  const [sim, setSim] = useState(null);
  const [config, setConfig] = useState(defaultConfig);
  const [stats, setStats] = useState(null);
  const [running, setRunning] = useState(false);
  const [speed, setSpeed] = useState(10);
  const [population, setPopulation] = useState(5000);
  const [activeTab, setActiveTab] = useState('overview');
  const [experimentResults, setExperimentResults] = useState(null);
  const animationRef = useRef(null);

  const initSimulation = useCallback(() => {
    console.log(`Initializing simulation with population: ${population}`);
    // Scale arrival rate with population (roughly 0.2% of population per tick, min 10, max 2000)
    // This ensures larger populations have proportionally more players coming online
    const scaledArrivalRate = Math.max(10, Math.min(2000, Math.round(population * 0.002)));
    const adjustedConfig = { ...config, arrivalRate: scaledArrivalRate };
    console.log(`Scaled arrival rate to: ${scaledArrivalRate} players/tick (${(scaledArrivalRate / population * 100).toFixed(3)}% of population)`);
    const newSim = new SimulationEngine(adjustedConfig, Date.now());
    newSim.generatePopulation(population);
    const stats = newSim.getStats();
    console.log(`Simulation initialized. Total players: ${stats.totalPlayers}, Arrival rate: ${scaledArrivalRate}/tick`);
    setSim(newSim);
    setStats(stats);
    setRunning(false);
    if (animationRef.current) cancelAnimationFrame(animationRef.current);
  }, [config, population]);

  useEffect(() => {
    initSimulation();
    return () => {
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    };
  }, [initSimulation]);

  useEffect(() => {
    if (!running || !sim) return;

    let lastTime = performance.now();
    const ticksPerFrame = speed;

    const animate = (now) => {
      const delta = now - lastTime;
      if (delta >= 50) {
        for (let i = 0; i < ticksPerFrame; i++) {
          sim.tick();
        }
        setStats(sim.getStats());
        lastTime = now;
      }
      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);
    return () => {
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    };
  }, [running, sim, speed]);

  const runExperiment = (paramName, values) => {
    const results = [];
    for (const value of values) {
      const testConfig = { ...config, [paramName]: value };
      const testSim = new SimulationEngine(testConfig, 42);
      testSim.generatePopulation(population);
      for (let i = 0; i < 500; i++) testSim.tick();
      const s = testSim.getStats();
      results.push({
        value,
        avgSearchTime: s.avgSearchTime,
        avgDeltaPing: s.avgDeltaPing,
        avgSkillDisparity: s.avgSkillDisparity,
        blowoutRate: s.blowoutRate * 100,
      });
    }
    setExperimentResults({ param: paramName, data: results });
  };

  const updateConfig = (key, value) => {
    setConfig(prev => {
      const newConfig = { ...prev, [key]: parseFloat(value) };
      // If arrival rate is being updated manually, don't auto-scale it
      if (key === 'arrivalRate') {
        return newConfig;
      }
      return newConfig;
    });
  };

  const formatTime = (seconds) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
  };

  if (!stats) return <div style={{ background: COLORS.dark, minHeight: '100vh', color: COLORS.text, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>Loading simulation...</div>;

  const bucketStats = sim?.getBucketStats() || [];
  const skillDist = sim?.getSkillDistribution() || [];
  const searchTimeHist = sim?.getSearchTimeHistogram() || [];
  const deltaPingHist = sim?.getDeltaPingHistogram() || [];

  return (
    <div style={{ 
      background: `linear-gradient(135deg, ${COLORS.darker} 0%, ${COLORS.dark} 50%, #0d1424 100%)`,
      minHeight: '100vh',
      color: COLORS.text,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    }}>
      {/* Header */}
      <header style={{
        background: `linear-gradient(90deg, ${COLORS.card}ee, ${COLORS.darker}ee)`,
        borderBottom: `1px solid ${COLORS.border}`,
        padding: '1rem 2rem',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        backdropFilter: 'blur(10px)',
      }}>
        <div>
          <h1 style={{ 
            margin: 0, 
            fontSize: '1.5rem',
            background: `linear-gradient(90deg, ${COLORS.primary}, ${COLORS.tertiary})`,
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            fontWeight: 700,
          }}>
            COD MATCHMAKING SIMULATOR
          </h1>
          <p style={{ margin: '0.25rem 0 0', color: COLORS.textMuted, fontSize: '0.75rem' }}>
            Research & Analysis Platform • Rust + WebAssembly Engine
          </p>
        </div>
        <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
          <span style={{ 
            background: running ? COLORS.success : COLORS.warning,
            padding: '0.25rem 0.75rem',
            borderRadius: '9999px',
            fontSize: '0.7rem',
            fontWeight: 600,
          }}>
            {running ? '● RUNNING' : '○ PAUSED'}
          </span>
          <span style={{ color: COLORS.textMuted, fontSize: '0.8rem' }}>
            T = {formatTime(stats.timeElapsed)}
          </span>
        </div>
      </header>

      <div style={{ display: 'flex', minHeight: 'calc(100vh - 64px)' }}>
        {/* Sidebar */}
        <aside style={{
          width: '280px',
          background: COLORS.card,
          borderRight: `1px solid ${COLORS.border}`,
          padding: '1rem',
          overflowY: 'auto',
        }}>
          <div style={{ marginBottom: '1.5rem' }}>
            <h3 style={{ fontSize: '0.7rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>SIMULATION</h3>
            <div style={{ display: 'flex', gap: '0.5rem', marginBottom: '0.75rem' }}>
              <button
                onClick={() => setRunning(!running)}
                style={{
                  flex: 1,
                  padding: '0.6rem',
                  background: running ? COLORS.danger : COLORS.primary,
                  border: 'none',
                  borderRadius: '6px',
                  color: COLORS.dark,
                  fontWeight: 600,
                  fontSize: '0.75rem',
                  cursor: 'pointer',
                  transition: 'transform 0.1s',
                }}
              >
                {running ? '⏸ PAUSE' : '▶ RUN'}
              </button>
              <button
                onClick={initSimulation}
                style={{
                  flex: 1,
                  padding: '0.6rem',
                  background: COLORS.cardHover,
                  border: `1px solid ${COLORS.border}`,
                  borderRadius: '6px',
                  color: COLORS.text,
                  fontWeight: 600,
                  fontSize: '0.75rem',
                  cursor: 'pointer',
                }}
              >
                ↻ RESET
              </button>
            </div>
            
            <label style={{ display: 'block', marginBottom: '0.5rem' }}>
              <span style={{ fontSize: '0.7rem', color: COLORS.textMuted }}>Speed: {speed}x</span>
              <input
                type="range"
                min="1"
                max="50"
                value={speed}
                onChange={(e) => setSpeed(parseInt(e.target.value))}
                style={{ width: '100%', accentColor: COLORS.primary }}
              />
            </label>
            
            <label style={{ display: 'block' }}>
              <span style={{ fontSize: '0.7rem', color: COLORS.textMuted }}>Population</span>
              <input
                type="number"
                value={population}
                onChange={(e) => setPopulation(parseInt(e.target.value) || 1000)}
                style={{
                  width: '100%',
                  padding: '0.5rem',
                  background: COLORS.darker,
                  border: `1px solid ${COLORS.border}`,
                  borderRadius: '4px',
                  color: COLORS.text,
                  fontSize: '0.85rem',
                }}
              />
            </label>
          </div>

          <div style={{ marginBottom: '1.5rem' }}>
            <h3 style={{ fontSize: '0.7rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>CONFIG PARAMS</h3>
            {[
              ['skillSimilarityInitial', 'Skill Similarity', 0, 0.3],
              ['skillSimilarityRate', 'Skill Backoff Rate', 0, 0.1],
              ['deltaPingInitial', 'Delta Ping Initial', 0, 50],
              ['deltaPingRate', 'Ping Backoff Rate', 0, 10],
              ['weightSkill', 'Skill Weight', 0, 1],
              ['weightGeo', 'Geo Weight', 0, 1],
              ['arrivalRate', 'Arrival Rate (auto-scaled)', 1, 2000],
            ].map(([key, label, min, max]) => (
              <label key={key} style={{ display: 'block', marginBottom: '0.5rem' }}>
                <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>{label}: {config[key].toFixed(2)}</span>
                <input
                  type="range"
                  min={min}
                  max={max}
                  step={(max - min) / 100}
                  value={config[key]}
                  onChange={(e) => updateConfig(key, e.target.value)}
                  style={{ width: '100%', accentColor: COLORS.tertiary }}
                />
              </label>
            ))}
          </div>

          <div>
            <h3 style={{ fontSize: '0.7rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>EXPERIMENTS</h3>
            <button
              onClick={() => runExperiment('skillSimilarityInitial', [0.01, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3])}
              style={{
                width: '100%',
                padding: '0.5rem',
                marginBottom: '0.5rem',
                background: COLORS.darker,
                border: `1px solid ${COLORS.border}`,
                borderRadius: '4px',
                color: COLORS.text,
                fontSize: '0.7rem',
                cursor: 'pointer',
              }}
            >
              Sweep: Skill Strictness
            </button>
            <button
              onClick={() => runExperiment('weightSkill', [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7])}
              style={{
                width: '100%',
                padding: '0.5rem',
                background: COLORS.darker,
                border: `1px solid ${COLORS.border}`,
                borderRadius: '4px',
                color: COLORS.text,
                fontSize: '0.7rem',
                cursor: 'pointer',
              }}
            >
              Sweep: Skill vs Ping Weight
            </button>
          </div>
        </aside>

        {/* Main Content */}
        <main style={{ flex: 1, padding: '1rem', overflowY: 'auto' }}>
          {/* Tabs */}
          <div style={{ display: 'flex', gap: '0.25rem', marginBottom: '1rem' }}>
            {['overview', 'distributions', 'buckets', 'experiments'].map(tab => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                style={{
                  padding: '0.5rem 1rem',
                  background: activeTab === tab ? COLORS.primary : 'transparent',
                  border: `1px solid ${activeTab === tab ? COLORS.primary : COLORS.border}`,
                  borderRadius: '4px',
                  color: activeTab === tab ? COLORS.dark : COLORS.textMuted,
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  cursor: 'pointer',
                  textTransform: 'uppercase',
                }}
              >
                {tab}
              </button>
            ))}
          </div>

          {/* Overview Tab */}
          {activeTab === 'overview' && (
            <div>
              {/* Stats Cards */}
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '0.75rem', marginBottom: '1rem' }}>
                {[
                  { label: 'Total Players', value: stats.totalPlayers || 0, color: COLORS.text, sub: `Population: ${population.toLocaleString()}` },
                  { label: 'Players Searching', value: stats.Searching, color: COLORS.warning },
                  { label: 'Players In Match', value: stats.InMatch, color: COLORS.success },
                  { label: 'Active Matches', value: stats.activeMatches, color: COLORS.tertiary },
                ].map(({ label, value, color, sub }) => (
                  <div key={label} style={{
                    background: COLORS.card,
                    border: `1px solid ${COLORS.border}`,
                    borderRadius: '8px',
                    padding: '1rem',
                  }}>
                    <div style={{ fontSize: '0.65rem', color: COLORS.textMuted, marginBottom: '0.25rem' }}>{label}</div>
                    <div style={{ fontSize: '1.5rem', fontWeight: 700, color }}>{value.toLocaleString()}</div>
                    {sub && <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginTop: '0.25rem' }}>{sub}</div>}
                  </div>
                ))}
              </div>

              {/* Key Metrics */}
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '0.75rem', marginBottom: '1rem' }}>
                {[
                  { label: 'Avg Search Time', value: `${stats.avgSearchTime.toFixed(1)}s`, sub: `P90: ${stats.searchTimeP90.toFixed(1)}s` },
                  { label: 'Avg Delta Ping', value: `${stats.avgDeltaPing.toFixed(1)}ms`, sub: `P90: ${stats.deltaPingP90.toFixed(1)}ms` },
                  { label: 'Skill Disparity', value: stats.avgSkillDisparity.toFixed(3), sub: 'Avg lobby spread' },
                  { label: 'Blowout Rate', value: `${(stats.blowoutRate * 100).toFixed(1)}%`, sub: 'Unbalanced matches' },
                ].map(({ label, value, sub }) => (
                  <div key={label} style={{
                    background: `linear-gradient(135deg, ${COLORS.card}, ${COLORS.darker})`,
                    border: `1px solid ${COLORS.border}`,
                    borderRadius: '8px',
                    padding: '1rem',
                  }}>
                    <div style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>{label}</div>
                    <div style={{ fontSize: '1.25rem', fontWeight: 600, color: COLORS.text }}>{value}</div>
                    <div style={{ fontSize: '0.6rem', color: COLORS.textMuted }}>{sub}</div>
                  </div>
                ))}
              </div>

              {/* Time Series Charts */}
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PLAYER STATES OVER TIME</h4>
                  <ResponsiveContainer width="100%" height={200}>
                    <LineChart data={stats.timeSeriesData}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="time" tick={{ fill: COLORS.textMuted, fontSize: 10 }} tickFormatter={(v) => `${Math.floor(v/60)}m`} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Line type="monotone" dataKey="searching" stroke={COLORS.warning} strokeWidth={2} dot={false} name="Searching" />
                      <Line type="monotone" dataKey="inMatch" stroke={COLORS.success} strokeWidth={2} dot={false} name="In Match" />
                    </LineChart>
                  </ResponsiveContainer>
                </div>

                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>QUALITY METRICS</h4>
                  <ResponsiveContainer width="100%" height={200}>
                    <LineChart data={stats.timeSeriesData}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="time" tick={{ fill: COLORS.textMuted, fontSize: 10 }} tickFormatter={(v) => `${Math.floor(v/60)}m`} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Line type="monotone" dataKey="avgSearchTime" stroke={COLORS.tertiary} strokeWidth={2} dot={false} name="Search Time (s)" />
                      <Line type="monotone" dataKey="avgDeltaPing" stroke={COLORS.secondary} strokeWidth={2} dot={false} name="Delta Ping (ms)" />
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              </div>
            </div>
          )}

          {/* Distributions Tab */}
          {activeTab === 'distributions' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>SKILL DISTRIBUTION</h4>
                <ResponsiveContainer width="100%" height={250}>
                  <BarChart data={skillDist}>
                    <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                    <XAxis dataKey="skill" tick={{ fill: COLORS.textMuted, fontSize: 9 }} interval={2} />
                    <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                    <Bar dataKey="count" fill={COLORS.primary} radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>

              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>SEARCH TIME DISTRIBUTION</h4>
                <ResponsiveContainer width="100%" height={250}>
                  <BarChart data={searchTimeHist}>
                    <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                    <XAxis dataKey="range" tick={{ fill: COLORS.textMuted, fontSize: 9 }} angle={-45} textAnchor="end" height={60} />
                    <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                    <Bar dataKey="count" fill={COLORS.tertiary} radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>

              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>DELTA PING DISTRIBUTION</h4>
                <ResponsiveContainer width="100%" height={250}>
                  <BarChart data={deltaPingHist}>
                    <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                    <XAxis dataKey="range" tick={{ fill: COLORS.textMuted, fontSize: 9 }} angle={-45} textAnchor="end" height={60} />
                    <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                    <Bar dataKey="count" fill={COLORS.secondary} radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>

              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PLAYER STATE BREAKDOWN</h4>
                <ResponsiveContainer width="100%" height={250}>
                  <BarChart data={[
                    { state: 'Offline', count: stats.Offline },
                    { state: 'In Lobby', count: stats.InLobby },
                    { state: 'Searching', count: stats.Searching },
                    { state: 'In Match', count: stats.InMatch },
                  ]}>
                    <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                    <XAxis dataKey="state" tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                    <Bar dataKey="count" radius={[4, 4, 0, 0]}>
                      {[COLORS.textMuted, COLORS.warning, COLORS.quaternary, COLORS.success].map((color, i) => (
                        <Cell key={i} fill={color} />
                      ))}
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}

          {/* Buckets Tab */}
          {activeTab === 'buckets' && (
            <div>
              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginBottom: '0.75rem' }}>
                <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>METRICS BY SKILL BUCKET</h4>
                <ResponsiveContainer width="100%" height={300}>
                  <BarChart data={bucketStats}>
                    <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                    <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Skill Bucket', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                    <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                    <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                    <Legend />
                    <Bar dataKey="avgSearchTime" name="Avg Search Time (s)" fill={COLORS.tertiary} radius={[2, 2, 0, 0]} />
                    <Bar dataKey="avgDeltaPing" name="Avg Delta Ping (ms)" fill={COLORS.secondary} radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>WIN RATE BY SKILL BUCKET</h4>
                  <ResponsiveContainer width="100%" height={200}>
                    <BarChart data={bucketStats}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} domain={[0, 1]} tickFormatter={(v) => `${(v*100).toFixed(0)}%`} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} formatter={(v) => `${(v*100).toFixed(1)}%`} />
                      <Bar dataKey="winRate" fill={COLORS.success} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>

                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PLAYERS PER BUCKET</h4>
                  <ResponsiveContainer width="100%" height={200}>
                    <BarChart data={bucketStats}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Bar dataKey="players" fill={COLORS.primary} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </div>
            </div>
          )}

          {/* Experiments Tab */}
          {activeTab === 'experiments' && (
            <div>
              <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginBottom: '1rem' }}>
                <h4 style={{ fontSize: '0.85rem', color: COLORS.text, marginBottom: '0.5rem' }}>Research Experiments</h4>
                <p style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '1rem' }}>
                  Run parameter sweeps to explore tradeoffs between search time, ping quality, skill matching, and fairness.
                  Use the experiment buttons in the sidebar to run a sweep, then analyze results here.
                </p>
                
                {!experimentResults && (
                  <div style={{ padding: '2rem', textAlign: 'center', color: COLORS.textMuted }}>
                    <p>No experiment results yet. Run an experiment from the sidebar.</p>
                    <p style={{ fontSize: '0.7rem', marginTop: '0.5rem' }}>
                      Try "Sweep: Skill Strictness" to see how SBMM intensity affects metrics.
                    </p>
                  </div>
                )}
              </div>

              {experimentResults && (
                <>
                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginBottom: '0.75rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>
                      PARAMETER SWEEP: {experimentResults.param}
                    </h4>
                    <ResponsiveContainer width="100%" height={300}>
                      <LineChart data={experimentResults.data}>
                        <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                        <XAxis dataKey="value" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: experimentResults.param, position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                        <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                        <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                        <Legend />
                        <Line type="monotone" dataKey="avgSearchTime" name="Search Time (s)" stroke={COLORS.tertiary} strokeWidth={2} />
                        <Line type="monotone" dataKey="avgDeltaPing" name="Delta Ping (ms)" stroke={COLORS.secondary} strokeWidth={2} />
                        <Line type="monotone" dataKey="avgSkillDisparity" name="Skill Disparity" stroke={COLORS.primary} strokeWidth={2} />
                        <Line type="monotone" dataKey="blowoutRate" name="Blowout Rate (%)" stroke={COLORS.warning} strokeWidth={2} />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>

                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>EXPERIMENT DATA TABLE</h4>
                    <div style={{ overflowX: 'auto' }}>
                      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.75rem' }}>
                        <thead>
                          <tr style={{ borderBottom: `1px solid ${COLORS.border}` }}>
                            <th style={{ textAlign: 'left', padding: '0.5rem', color: COLORS.textMuted }}>Value</th>
                            <th style={{ textAlign: 'right', padding: '0.5rem', color: COLORS.textMuted }}>Search Time</th>
                            <th style={{ textAlign: 'right', padding: '0.5rem', color: COLORS.textMuted }}>Delta Ping</th>
                            <th style={{ textAlign: 'right', padding: '0.5rem', color: COLORS.textMuted }}>Skill Disparity</th>
                            <th style={{ textAlign: 'right', padding: '0.5rem', color: COLORS.textMuted }}>Blowout Rate</th>
                          </tr>
                        </thead>
                        <tbody>
                          {experimentResults.data.map((row, i) => (
                            <tr key={i} style={{ borderBottom: `1px solid ${COLORS.border}22` }}>
                              <td style={{ padding: '0.5rem' }}>{row.value.toFixed(3)}</td>
                              <td style={{ padding: '0.5rem', textAlign: 'right' }}>{row.avgSearchTime.toFixed(1)}s</td>
                              <td style={{ padding: '0.5rem', textAlign: 'right' }}>{row.avgDeltaPing.toFixed(1)}ms</td>
                              <td style={{ padding: '0.5rem', textAlign: 'right' }}>{row.avgSkillDisparity.toFixed(4)}</td>
                              <td style={{ padding: '0.5rem', textAlign: 'right' }}>{row.blowoutRate.toFixed(1)}%</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </>
              )}
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
