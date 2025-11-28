import React, { useState, useEffect, useCallback, useRef } from 'react';
import { LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, ScatterChart, Scatter, Cell, RadarChart, PolarGrid, PolarAngleAxis, PolarRadiusAxis, Radar } from 'recharts';
import init, { SimulationEngine } from './wasm/cod_matchmaking_sim.js';

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
  // Fraction of players that participate in parties (0-1).
  // Defaults to ~50% to match the roadmap's party experiments.
  partyPlayerFraction: 0.5,
  tickInterval: 5,
  numSkillBuckets: 10,
  topKCandidates: 50,
  arrivalRate: 10,
  useExactTeamBalancing: true,
  gamma: 2.0,
  blowoutSkillCoefficient: 0.4,
  blowoutImbalanceCoefficient: 0.3,
  blowoutMildThreshold: 0.15,
  blowoutModerateThreshold: 0.35,
  blowoutSevereThreshold: 0.6,
  skillLearningRate: 0.01,
  performanceNoiseStd: 0.15,
  enableSkillEvolution: true,
  skillUpdateBatchSize: 10,
  // Retention model configuration
  retentionConfig: {
    thetaPing: -0.02,
    thetaSearchTime: -0.015,
    thetaBlowout: -0.5,
    thetaWinRate: 0.8,
    thetaPerformance: 0.6,
    baseContinueProb: 0.0,
    experienceWindowSize: 5,
  },
};

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
  const [wasmReady, setWasmReady] = useState(false);
  const [wasmError, setWasmError] = useState(null);
  const [parties, setParties] = useState([]);
  const [skillEvolutionData, setSkillEvolutionData] = useState([]);
  const [performanceDistribution, setPerformanceDistribution] = useState([]);
  const [retentionStats, setRetentionStats] = useState(null);
  const [sessionStats, setSessionStats] = useState(null);
  const [effectivePopulationHistory, setEffectivePopulationHistory] = useState([]);
  const [returnStats, setReturnStats] = useState(null);
  const animationRef = useRef(null);

  // Initialize WASM on mount
  useEffect(() => {
    let mounted = true;
    init()
      .then(() => {
        if (mounted) {
          setWasmReady(true);
          setWasmError(null);
        }
      })
      .catch((err) => {
        console.error('Failed to initialize WASM:', err);
        if (mounted) {
          setWasmError(err.message);
          setWasmReady(false);
        }
      });
    return () => {
      mounted = false;
    };
  }, []);

  // Convert JS config to Rust config format
  const convertConfigToRust = useCallback((jsConfig) => {
    return {
      max_ping: jsConfig.maxPing,
      delta_ping_initial: jsConfig.deltaPingInitial,
      delta_ping_rate: jsConfig.deltaPingRate,
      delta_ping_max: jsConfig.deltaPingMax,
      skill_similarity_initial: jsConfig.skillSimilarityInitial,
      skill_similarity_rate: jsConfig.skillSimilarityRate,
      skill_similarity_max: jsConfig.skillSimilarityMax,
      max_skill_disparity_initial: jsConfig.maxSkillDisparityInitial,
      max_skill_disparity_rate: jsConfig.maxSkillDisparityRate,
      max_skill_disparity_max: jsConfig.maxSkillDisparityMax,
      weight_geo: jsConfig.weightGeo,
      weight_skill: jsConfig.weightSkill,
      weight_input: jsConfig.weightInput,
      weight_platform: jsConfig.weightPlatform,
      party_player_fraction: jsConfig.partyPlayerFraction ?? 0.5,
      quality_weight_ping: 0.4,
      quality_weight_skill_balance: 0.4,
      quality_weight_wait_time: 0.2,
      tick_interval: jsConfig.tickInterval,
      num_skill_buckets: jsConfig.numSkillBuckets,
      top_k_candidates: jsConfig.topKCandidates,
      use_exact_team_balancing: jsConfig.useExactTeamBalancing ?? true,
      gamma: jsConfig.gamma ?? 2.0,
      blowout_skill_coefficient: jsConfig.blowoutSkillCoefficient ?? 0.4,
      blowout_imbalance_coefficient: jsConfig.blowoutImbalanceCoefficient ?? 0.3,
      blowout_mild_threshold: jsConfig.blowoutMildThreshold ?? 0.15,
      blowout_moderate_threshold: jsConfig.blowoutModerateThreshold ?? 0.35,
      blowout_severe_threshold: jsConfig.blowoutSevereThreshold ?? 0.6,
      skill_learning_rate: jsConfig.skillLearningRate ?? 0.01,
      performance_noise_std: jsConfig.performanceNoiseStd ?? 0.15,
      enable_skill_evolution: jsConfig.enableSkillEvolution ?? true,
      skill_update_batch_size: jsConfig.skillUpdateBatchSize ?? 10,
      retention_config: {
        theta_ping: jsConfig.retentionConfig?.thetaPing ?? -0.02,
        theta_search_time: jsConfig.retentionConfig?.thetaSearchTime ?? -0.015,
        theta_blowout: jsConfig.retentionConfig?.thetaBlowout ?? -0.5,
        theta_win_rate: jsConfig.retentionConfig?.thetaWinRate ?? 0.8,
        theta_performance: jsConfig.retentionConfig?.thetaPerformance ?? 0.6,
        base_continue_prob: jsConfig.retentionConfig?.baseContinueProb ?? 0.0,
        experience_window_size: jsConfig.retentionConfig?.experienceWindowSize ?? 5,
      },
    };
  }, []);

  const initSimulation = useCallback(() => {
    if (!wasmReady) {
      console.log('WASM not ready yet, waiting...');
      return;
    }

    try {
      console.log(`Initializing simulation with population: ${population}`);
      // Scale arrival rate with population (roughly 0.2% of population per tick, min 10, max 2000)
      const scaledArrivalRate = Math.max(10, Math.min(2000, Math.round(population * 0.002)));
      const adjustedConfig = { ...config, arrivalRate: scaledArrivalRate };
      console.log(`Scaled arrival rate to: ${scaledArrivalRate} players/tick`);
      
      // Create WASM simulation
      const rustConfig = convertConfigToRust(adjustedConfig);
      const newSim = new SimulationEngine(BigInt(Date.now()));
      newSim.update_config(JSON.stringify(rustConfig));
      newSim.generate_population(population);
      newSim.set_arrival_rate(scaledArrivalRate);
      
      // Get initial stats
      const statsJson = newSim.get_stats();
      const rawStats = JSON.parse(statsJson);
      // Map Rust field names to JS field names for compatibility
      const stats = {
        currentTime: rawStats.ticks,
        timeElapsed: rawStats.time_elapsed,
        totalPlayers: rawStats.players_offline + rawStats.players_in_lobby + rawStats.players_searching + rawStats.players_in_match,
        Offline: rawStats.players_offline,
        InLobby: rawStats.players_in_lobby,
        Searching: rawStats.players_searching,
        InMatch: rawStats.players_in_match,
        activeMatches: rawStats.active_matches,
        totalMatches: rawStats.total_matches,
        avgSearchTime: rawStats.avg_search_time,
        searchTimeP50: rawStats.search_time_p50,
        searchTimeP90: rawStats.search_time_p90,
        searchTimeP99: rawStats.search_time_p99,
        avgDeltaPing: rawStats.avg_delta_ping,
        deltaPingP50: rawStats.delta_ping_p50,
        deltaPingP90: rawStats.delta_ping_p90,
        avgSkillDisparity: rawStats.avg_skill_disparity,
        blowoutRate: rawStats.blowout_rate,
        // Party metrics
        partyCount: rawStats.party_count || 0,
        avgPartySize: rawStats.avg_party_size || 0,
        partyMatchCount: rawStats.party_match_count || 0,
        soloMatchCount: rawStats.solo_match_count || 0,
        partySearchTimes: rawStats.party_search_times || [],
        soloSearchTimes: rawStats.solo_search_times || [],
        // Slice C: Blowout and team balancing metrics
        blowoutSeverityCounts: rawStats.blowout_severity_counts || {},
        perPlaylistBlowoutRate: rawStats.per_playlist_blowout_rate || {},
        teamSkillDifferenceSamples: rawStats.team_skill_difference_samples || [],
        // Slice D: Skill evolution metrics
        skillEvolutionEnabled: rawStats.skill_evolution_enabled || false,
        totalSkillUpdates: rawStats.total_skill_updates || 0,
        // Slice E: Retention metrics
        perBucketContinueRate: rawStats.per_bucket_continue_rate || {},
        avgMatchesPerSession: rawStats.avg_matches_per_session || 0,
        sessionLengthDistribution: rawStats.session_length_distribution || [],
        activeSessions: rawStats.active_sessions || 0,
        totalSessionsCompleted: rawStats.total_sessions_completed || 0,
        // Population health
        populationChangeRate: rawStats.population_change_rate || 0,
        // Time series data
        timeSeriesData: [],
      };
      console.log(`Simulation initialized. Total players: ${stats.totalPlayers}, Arrival rate: ${scaledArrivalRate}/tick`);
      
      // Fetch initial skill evolution data
      try {
        const evolutionJson = newSim.get_skill_evolution_data();
        const evolutionData = JSON.parse(evolutionJson);
        setSkillEvolutionData(evolutionData);
        
        const perfDistJson = newSim.get_performance_distribution(20);
        const perfDist = JSON.parse(perfDistJson);
        setPerformanceDistribution(perfDist);
      } catch (e) {
        console.error('Error fetching initial skill evolution data:', e);
      }
      
      setSim(newSim);
      setStats(stats);
      setRunning(false);
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    } catch (error) {
      console.error('Failed to initialize simulation:', error);
      setWasmError(error.message);
    }
  }, [config, population, wasmReady, convertConfigToRust]);

  useEffect(() => {
    if (wasmReady) {
      initSimulation();
    }
    return () => {
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    };
  }, [initSimulation, wasmReady]);

  useEffect(() => {
    if (!running || !sim || !wasmReady) return;

    let lastTime = performance.now();
    const ticksPerFrame = speed;

    const animate = (now) => {
      const delta = now - lastTime;
      if (delta >= 50) {
        try {
          for (let i = 0; i < ticksPerFrame; i++) {
            sim.tick();
          }
          const statsJson = sim.get_stats();
          const rawStats = JSON.parse(statsJson);
          // Get previous stats for time series
          setStats(prevStats => {
            // Map Rust field names to JS field names for compatibility
            const newStats = {
              currentTime: rawStats.ticks,
              timeElapsed: rawStats.time_elapsed,
              totalPlayers: rawStats.players_offline + rawStats.players_in_lobby + rawStats.players_searching + rawStats.players_in_match,
              Offline: rawStats.players_offline,
              InLobby: rawStats.players_in_lobby,
              Searching: rawStats.players_searching,
              InMatch: rawStats.players_in_match,
              activeMatches: rawStats.active_matches,
              totalMatches: rawStats.total_matches,
              avgSearchTime: rawStats.avg_search_time,
              searchTimeP50: rawStats.search_time_p50,
              searchTimeP90: rawStats.search_time_p90,
              searchTimeP99: rawStats.search_time_p99,
              avgDeltaPing: rawStats.avg_delta_ping,
              deltaPingP50: rawStats.delta_ping_p50,
              deltaPingP90: rawStats.delta_ping_p90,
              avgSkillDisparity: rawStats.avg_skill_disparity,
              blowoutRate: rawStats.blowout_rate,
              // Party metrics
              partyCount: rawStats.party_count || 0,
              avgPartySize: rawStats.avg_party_size || 0,
              partyMatchCount: rawStats.party_match_count || 0,
              soloMatchCount: rawStats.solo_match_count || 0,
              partySearchTimes: rawStats.party_search_times || [],
              soloSearchTimes: rawStats.solo_search_times || [],
              // Slice C: Blowout and team balancing metrics
              blowoutSeverityCounts: rawStats.blowout_severity_counts || {},
              perPlaylistBlowoutRate: rawStats.per_playlist_blowout_rate || {},
              teamSkillDifferenceSamples: rawStats.team_skill_difference_samples || [],
              // Slice D: Skill evolution metrics
              skillEvolutionEnabled: rawStats.skill_evolution_enabled || false,
              totalSkillUpdates: rawStats.total_skill_updates || 0,
              // Slice E: Retention metrics
              perBucketContinueRate: rawStats.per_bucket_continue_rate || {},
              avgMatchesPerSession: rawStats.avg_matches_per_session || 0,
              sessionLengthDistribution: rawStats.session_length_distribution || [],
              activeSessions: rawStats.active_sessions || 0,
              totalSessionsCompleted: rawStats.total_sessions_completed || 0,
              populationChangeRate: prevStats?.populationChangeRate ?? 0,
              // Time series data (preserve from previous or initialize)
              timeSeriesData: prevStats?.timeSeriesData || [],
            };
            
            // Fetch skill evolution data and performance distribution
            try {
              if (sim) {
                const evolutionJson = sim.get_skill_evolution_data();
                const evolutionData = JSON.parse(evolutionJson);
                setSkillEvolutionData(evolutionData);
                
                const perfDistJson = sim.get_performance_distribution(20);
                const perfDist = JSON.parse(perfDistJson);
                setPerformanceDistribution(perfDist);
                
                // Fetch retention and session stats
                const retentionJson = sim.get_retention_stats();
                const retention = JSON.parse(retentionJson);
                setRetentionStats(retention);
                
                // Update population change rate in stats
                setStats(prev => ({ 
                  ...prev, 
                  populationChangeRate: retention?.population_change_rate ?? prev.populationChangeRate ?? 0 
                }));
                
                const sessionJson = sim.get_session_stats();
                const session = JSON.parse(sessionJson);
                setSessionStats(session);
                
                // Fetch return probability and population health stats
                const returnJson = sim.get_return_stats();
                const returnData = JSON.parse(returnJson);
                setReturnStats(returnData);
                
                const populationHistoryJson = sim.get_effective_population_history();
                const populationHistory = JSON.parse(populationHistoryJson);
                setEffectivePopulationHistory(populationHistory);
              }
            } catch (e) {
              console.error('Error fetching skill evolution data:', e);
            }
            // Update time series
            if (newStats.timeSeriesData.length === 0 || newStats.timeSeriesData[newStats.timeSeriesData.length - 1].time !== newStats.timeElapsed) {
              newStats.timeSeriesData.push({
                time: newStats.timeElapsed,
                searching: newStats.Searching,
                inMatch: newStats.InMatch,
                inLobby: newStats.InLobby,
                activeMatches: newStats.activeMatches,
                avgSearchTime: newStats.avgSearchTime,
                avgDeltaPing: newStats.avgDeltaPing,
              });
              if (newStats.timeSeriesData.length > 200) {
                newStats.timeSeriesData.shift();
              }
            }
            return newStats;
          });
        } catch (error) {
          console.error('Error during simulation tick:', error);
          setWasmError(error.message);
          setRunning(false);
        }
        lastTime = now;
      }
      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);
    return () => {
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    };
  }, [running, sim, speed, wasmReady]);

  const runExperiment = (paramName, values) => {
    if (!wasmReady) {
      console.error('WASM not ready for experiments');
      return;
    }
    
    const results = [];
    for (const value of values) {
      const testConfig = { ...config, [paramName]: value };
      const rustConfig = convertConfigToRust(testConfig);
      const testSim = new SimulationEngine(BigInt(42));
      testSim.update_config(JSON.stringify(rustConfig));
      testSim.generate_population(population);
      for (let i = 0; i < 500; i++) testSim.tick();
      
      const statsJson = testSim.get_stats();
      const rawStats = JSON.parse(statsJson);
      const s = {
        avgSearchTime: rawStats.avg_search_time,
        avgDeltaPing: rawStats.avg_delta_ping,
        avgSkillDisparity: rawStats.avg_skill_disparity,
        blowoutRate: rawStats.blowout_rate,
      };
      
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
      let newConfig;
      // Handle nested objects (e.g., retentionConfig)
      if (key === 'retentionConfig' && typeof value === 'object') {
        newConfig = { ...prev, retentionConfig: { ...prev.retentionConfig, ...value } };
      } else {
        // Handle simple key-value pairs
        newConfig = { ...prev, [key]: typeof value === 'number' ? value : parseFloat(value) };
      }
      
      // If arrival rate is being updated manually, don't auto-scale it
      if (key === 'arrivalRate') {
        // Update WASM sim if it exists
        if (sim && wasmReady) {
          try {
            sim.set_arrival_rate(parseFloat(value));
          } catch (error) {
            console.error('Failed to update arrival rate:', error);
          }
        }
        return newConfig;
      }
      
      // Update WASM sim config for all other changes
      if (sim && wasmReady) {
        try {
          const rustConfig = convertConfigToRust(newConfig);
          if (key === 'retentionConfig') {
            console.log('Updating retention config:', JSON.stringify(newConfig.retentionConfig, null, 2));
          }
          sim.update_config(JSON.stringify(rustConfig));
        } catch (error) {
          console.error('Failed to update config:', error);
        }
      }
      return newConfig;
    });
  };

  const formatTime = (seconds) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
  };

  // Refresh parties list
  const refreshParties = useCallback(() => {
    if (!sim || !wasmReady) return;
    try {
      const partiesJson = sim.get_parties();
      const partiesList = JSON.parse(partiesJson);
      setParties(partiesList);
    } catch (error) {
      console.error('Error refreshing parties:', error);
    }
  }, [sim, wasmReady]);

  // Refresh parties when stats update (with debouncing to avoid recursive errors)
  useEffect(() => {
    if (sim && wasmReady && stats) {
      // Use setTimeout to avoid calling during active simulation ticks
      const timeoutId = setTimeout(() => {
        try {
          refreshParties();
        } catch (error) {
          console.error('Error in parties refresh effect:', error);
        }
      }, 100);
      return () => clearTimeout(timeoutId);
    }
  }, [sim, wasmReady, stats, refreshParties]);

  // Loading/error states
  if (!wasmReady && !wasmError) {
    return <div style={{ background: COLORS.dark, minHeight: '100vh', color: COLORS.text, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: '1rem' }}>
      <div>Loading WASM module...</div>
      <div style={{ fontSize: '0.75rem', color: COLORS.textMuted }}>Initializing Rust simulation engine</div>
    </div>;
  }

  if (wasmError) {
    return <div style={{ background: COLORS.dark, minHeight: '100vh', color: COLORS.text, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: '1rem' }}>
      <div style={{ color: COLORS.danger }}>WASM Error: {wasmError}</div>
      <button onClick={() => window.location.reload()} style={{ padding: '0.5rem 1rem', background: COLORS.primary, border: 'none', borderRadius: '4px', color: COLORS.dark, cursor: 'pointer' }}>
        Reload Page
      </button>
    </div>;
  }

  if (!stats) return <div style={{ background: COLORS.dark, minHeight: '100vh', color: COLORS.text, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>Loading simulation...</div>;

  // Get stats data from WASM
  let bucketStats = [];
  let skillDist = [];
  let searchTimeHist = [];
  let deltaPingHist = [];
  
  if (sim && wasmReady) {
    try {
      const bucketStatsJson = sim.get_bucket_stats();
      bucketStats = JSON.parse(bucketStatsJson);
      bucketStats = Object.values(bucketStats).map(b => ({
        bucket: b.bucket_id,
        players: b.player_count,
        avgSearchTime: b.avg_search_time,
        avgDeltaPing: b.avg_delta_ping,
        winRate: b.win_rate,
        matches: b.matches_played,
      }));

      const skillDistJson = sim.get_skill_distribution();
      skillDist = JSON.parse(skillDistJson).map(([skill, count]) => ({ skill: skill.toFixed(2), count }));

      const searchTimeHistJson = sim.get_search_time_histogram(15);
      searchTimeHist = JSON.parse(searchTimeHistJson).map(bin => ({
        range: `${bin.bin_start.toFixed(0)}-${bin.bin_end.toFixed(0)}s`,
        count: bin.count,
      }));

      const deltaPingHistJson = sim.get_delta_ping_histogram(12);
      deltaPingHist = JSON.parse(deltaPingHistJson).map(bin => ({
        range: `${bin.bin_start.toFixed(0)}-${bin.bin_end.toFixed(0)}ms`,
        count: bin.count,
      }));
    } catch (error) {
      console.error('Error getting stats data:', error);
    }
  }

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
              {config.enableSkillEvolution !== undefined && (
                <button
                  onClick={() => {
                    if (sim && wasmReady) {
                      try {
                        const newValue = !config.enableSkillEvolution;
                        updateConfig('enableSkillEvolution', newValue);
                        sim.toggle_skill_evolution(newValue);
                      } catch (error) {
                        console.error('Error toggling skill evolution:', error);
                      }
                    }
                  }}
                  style={{ 
                    flex: 1,
                    padding: '0.6rem', 
                    background: config.enableSkillEvolution ? COLORS.success : COLORS.darker, 
                    border: `1px solid ${config.enableSkillEvolution ? COLORS.success : COLORS.border}`, 
                    borderRadius: '6px', 
                    color: COLORS.text, 
                    cursor: 'pointer', 
                    fontSize: '0.7rem',
                    fontWeight: 600,
                  }}
                >
                  {config.enableSkillEvolution ? '✓ EVOLVING' : '○ STATIC'}
                </button>
              )}
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
            ['partyPlayerFraction', 'Party Player Fraction', 0, 1],
              ['arrivalRate', 'Arrival Rate (auto-scaled)', 1, 2000],
            ].map(([key, label, min, max]) => (
              <label key={key} style={{ display: 'block', marginBottom: '0.5rem' }}>
                <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                  {label}: {config[key].toFixed(2)}
                  {key === 'partyPlayerFraction' && ' (0 = all solo, 1 = all in parties)'}
                </span>
                <input
                  type="range"
                  min={min}
                  max={max}
                  step={(max - min) / 100}
                  value={config[key]}
                  onChange={(e) => updateConfig(key, parseFloat(e.target.value))}
                  style={{ width: '100%', accentColor: COLORS.tertiary }}
                />
              </label>
            ))}
            <div style={{ marginTop: '1rem', paddingTop: '1rem', borderTop: `1px solid ${COLORS.border}` }}>
              <h4 style={{ fontSize: '0.65rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>TEAM BALANCING & BLOWOUTS</h4>
              <label style={{ display: 'block', marginBottom: '0.5rem' }}>
                <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                  <input
                    type="checkbox"
                    checked={config.useExactTeamBalancing}
                    onChange={(e) => updateConfig('useExactTeamBalancing', e.target.checked)}
                    style={{ marginRight: '0.5rem', accentColor: COLORS.tertiary }}
                  />
                  Use Exact Team Balancing (6v6)
                </span>
              </label>
              {[
                ['gamma', 'Win Probability Gamma', 0.5, 5.0],
                ['blowoutSkillCoefficient', 'Blowout Skill Coeff', 0.0, 1.0],
                ['blowoutImbalanceCoefficient', 'Blowout Imbalance Coeff', 0.0, 1.0],
                ['blowoutMildThreshold', 'Blowout Mild Threshold', 0.0, 1.0],
                ['blowoutModerateThreshold', 'Blowout Moderate Threshold', 0.0, 1.0],
                ['blowoutSevereThreshold', 'Blowout Severe Threshold', 0.0, 1.0],
              ].map(([key, label, min, max]) => (
                <label key={key} style={{ display: 'block', marginBottom: '0.5rem' }}>
                  <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                    {label}: {config[key]?.toFixed(2) ?? '0.00'}
                  </span>
                  <input
                    type="range"
                    min={min}
                    max={max}
                    step={(max - min) / 100}
                    value={config[key] ?? (min + max) / 2}
                    onChange={(e) => updateConfig(key, parseFloat(e.target.value))}
                    style={{ width: '100%', accentColor: COLORS.tertiary }}
                  />
                </label>
              ))}
            </div>
            <div style={{ marginTop: '1rem', paddingTop: '1rem', borderTop: `1px solid ${COLORS.border}` }}>
              <h4 style={{ fontSize: '0.65rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>SKILL EVOLUTION</h4>
              <label style={{ display: 'block', marginBottom: '0.5rem' }}>
                <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                  <input
                    type="checkbox"
                    checked={config.enableSkillEvolution}
                    onChange={(e) => updateConfig('enableSkillEvolution', e.target.checked)}
                    style={{ marginRight: '0.5rem', accentColor: COLORS.tertiary }}
                  />
                  Enable Skill Evolution
                </span>
              </label>
              {[
                ['skillLearningRate', 'Skill Learning Rate (α)', 0.001, 0.1],
                ['performanceNoiseStd', 'Performance Noise Std (σ)', 0.05, 0.5],
                ['skillUpdateBatchSize', 'Skill Update Batch Size', 1, 50],
              ].map(([key, label, min, max]) => (
                <label key={key} style={{ display: 'block', marginBottom: '0.5rem' }}>
                  <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                    {label}: {config[key]?.toFixed(key === 'skillUpdateBatchSize' ? 0 : 3) ?? (key === 'skillUpdateBatchSize' ? '10' : '0.01')}
                  </span>
                  <input
                    type="range"
                    min={min}
                    max={max}
                    step={key === 'skillUpdateBatchSize' ? 1 : (max - min) / 100}
                    value={config[key] ?? (key === 'skillUpdateBatchSize' ? 10 : (min + max) / 2)}
                    onChange={(e) => updateConfig(key, key === 'skillUpdateBatchSize' ? parseInt(e.target.value) : parseFloat(e.target.value))}
                    style={{ width: '100%', accentColor: COLORS.tertiary }}
                  />
                </label>
              ))}
            </div>
            <div style={{ marginTop: '1rem', paddingTop: '1rem', borderTop: `1px solid ${COLORS.border}` }}>
              <h4 style={{ fontSize: '0.65rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>RETENTION MODEL</h4>
              <div style={{ marginBottom: '0.75rem', display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                {[
                  { name: 'Ping-First', config: { thetaPing: -0.05, thetaSearchTime: -0.01, thetaBlowout: -0.3, thetaWinRate: 0.5, thetaPerformance: 0.4 } },
                  { name: 'Skill-First', config: { thetaPing: -0.01, thetaSearchTime: -0.01, thetaBlowout: -0.3, thetaWinRate: 1.2, thetaPerformance: 1.0 } },
                  { name: 'Lenient', config: { thetaPing: -0.01, thetaSearchTime: -0.005, thetaBlowout: -0.2, thetaWinRate: 0.6, thetaPerformance: 0.5 } },
                  { name: 'Strict', config: { thetaPing: -0.03, thetaSearchTime: -0.025, thetaBlowout: -0.8, thetaWinRate: 1.0, thetaPerformance: 0.8 } },
                ].map((preset) => (
                  <button
                    key={preset.name}
                    onClick={() => {
                      const newConfig = {
                        ...config,
                        retentionConfig: {
                          ...config.retentionConfig,
                          ...preset.config,
                        },
                      };
                      updateConfig('retentionConfig', newConfig.retentionConfig);
                    }}
                    style={{
                      padding: '0.25rem 0.5rem',
                      fontSize: '0.6rem',
                      background: COLORS.card,
                      border: `1px solid ${COLORS.border}`,
                      color: COLORS.text,
                      cursor: 'pointer',
                      borderRadius: '4px',
                    }}
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
              {[
                ['retentionConfig.thetaPing', 'Theta Ping', -0.1, 0.0],
                ['retentionConfig.thetaSearchTime', 'Theta Search Time', -0.05, 0.0],
                ['retentionConfig.thetaBlowout', 'Theta Blowout', -1.0, 0.0],
                ['retentionConfig.thetaWinRate', 'Theta Win Rate', 0.0, 2.0],
                ['retentionConfig.thetaPerformance', 'Theta Performance', 0.0, 2.0],
                ['retentionConfig.baseContinueProb', 'Base Continue Prob', -2.0, 2.0],
                ['retentionConfig.experienceWindowSize', 'Experience Window Size', 1, 20],
              ].map(([key, label, min, max]) => {
                const fieldName = key.split('.')[1]; // Extract field name (e.g., 'thetaPing')
                const value = config.retentionConfig?.[fieldName];
                const currentValue = typeof value === 'number' ? value : (key.includes('WindowSize') ? 5 : 0);
                // Clamp value to valid range for display
                const clampedValue = Math.max(min, Math.min(max, currentValue));
                return (
                  <label key={key} style={{ display: 'block', marginBottom: '0.5rem' }}>
                    <span style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>
                      {label}: {key.includes('WindowSize') ? clampedValue.toFixed(0) : clampedValue.toFixed(3)}
                    </span>
                    <input
                      type="range"
                      min={min}
                      max={max}
                      step={key.includes('WindowSize') ? 1 : (max - min) / 200}
                      value={clampedValue}
                      onChange={(e) => {
                        const newValue = key.includes('WindowSize') ? parseInt(e.target.value) : parseFloat(e.target.value);
                        const newRetentionConfig = { ...config.retentionConfig, [fieldName]: newValue };
                        updateConfig('retentionConfig', newRetentionConfig);
                      }}
                      style={{ width: '100%', accentColor: COLORS.tertiary }}
                    />
                  </label>
                );
              })}
            </div>
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
                  { label: 'Effective Population', value: (stats.InLobby || 0) + (stats.Searching || 0) + (stats.InMatch || 0), color: COLORS.primary, sub: 'Concurrent (active)' },
                  { label: 'Players Searching', value: stats.Searching, color: COLORS.warning },
                  { label: 'Players In Match', value: stats.InMatch, color: COLORS.success },
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
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '0.75rem', marginBottom: '1rem' }}>
                {[
                  { label: 'Avg Search Time', value: `${stats.avgSearchTime.toFixed(1)}s`, sub: `P90: ${stats.searchTimeP90.toFixed(1)}s` },
                  { label: 'Avg Delta Ping', value: `${stats.avgDeltaPing.toFixed(1)}ms`, sub: `P90: ${stats.deltaPingP90.toFixed(1)}ms` },
                  { label: 'Skill Disparity', value: stats.avgSkillDisparity.toFixed(3), sub: 'Avg lobby spread' },
                  { label: 'Blowout Rate', value: `${(stats.blowoutRate * 100).toFixed(1)}%`, sub: 'Unbalanced matches' },
                  { 
                    label: 'Population Change', 
                    value: `${((stats?.populationChangeRate || 0) >= 0 ? '+' : '')}${(stats?.populationChangeRate || 0).toFixed(2)}/s`, 
                    sub: (stats?.populationChangeRate || 0) >= 0 ? 'Growing' : 'Shrinking',
                    color: (stats?.populationChangeRate || 0) >= 0 ? COLORS.success : COLORS.danger
                  },
                ].map(({ label, value, sub, color }) => (
                  <div key={label} style={{
                    background: `linear-gradient(135deg, ${COLORS.card}, ${COLORS.darker})`,
                    border: `1px solid ${COLORS.border}`,
                    borderRadius: '8px',
                    padding: '1rem',
                  }}>
                    <div style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>{label}</div>
                    <div style={{ fontSize: '1.25rem', fontWeight: 600, color: color || COLORS.text }}>{value}</div>
                    <div style={{ fontSize: '0.6rem', color: COLORS.textMuted }}>{sub}</div>
                  </div>
                ))}
              </div>

              {/* Party Metrics Section */}
              {(stats.partyCount !== undefined || stats.partyMatchCount !== undefined) && (
                <div style={{ 
                  background: COLORS.card, 
                  border: `1px solid ${COLORS.border}`, 
                  borderRadius: '8px', 
                  padding: '1rem',
                  marginBottom: '1rem',
                }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>PARTY STATISTICS</h4>
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '0.75rem' }}>
                    {[
                      { label: 'Active Parties', value: stats.partyCount || 0, color: COLORS.primary },
                      { label: 'Avg Party Size', value: stats.avgPartySize ? stats.avgPartySize.toFixed(1) : '0.0', color: COLORS.tertiary },
                      { label: 'Party Matches', value: stats.partyMatchCount || 0, color: COLORS.success, sub: stats.totalMatches ? `${((stats.partyMatchCount || 0) / stats.totalMatches * 100).toFixed(1)}%` : '0%' },
                      { label: 'Solo Matches', value: stats.soloMatchCount || 0, color: COLORS.warning, sub: stats.totalMatches ? `${((stats.soloMatchCount || 0) / stats.totalMatches * 100).toFixed(1)}%` : '0%' },
                    ].map(({ label, value, color, sub }) => (
                      <div key={label} style={{
                        background: COLORS.darker,
                        border: `1px solid ${COLORS.border}`,
                        borderRadius: '6px',
                        padding: '0.75rem',
                      }}>
                        <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginBottom: '0.25rem' }}>{label}</div>
                        <div style={{ fontSize: '1.1rem', fontWeight: 600, color }}>{value}</div>
                        {sub && <div style={{ fontSize: '0.55rem', color: COLORS.textMuted, marginTop: '0.25rem' }}>{sub}</div>}
                      </div>
                    ))}
                  </div>
                  {(stats.partySearchTimes && stats.partySearchTimes.length > 0) && (
                    <div style={{ marginTop: '0.75rem', fontSize: '0.65rem', color: COLORS.textMuted }}>
                      Avg Party Search Time: {(stats.partySearchTimes.reduce((a, b) => a + b, 0) / stats.partySearchTimes.length).toFixed(1)}s
                      {stats.soloSearchTimes && stats.soloSearchTimes.length > 0 && (
                        <span style={{ marginLeft: '1rem' }}>
                          Avg Solo Search Time: {(stats.soloSearchTimes.reduce((a, b) => a + b, 0) / stats.soloSearchTimes.length).toFixed(1)}s
                        </span>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* Skill Evolution Metrics Section */}
              {config.enableSkillEvolution && stats && (
                <div style={{ 
                  background: COLORS.card, 
                  border: `1px solid ${COLORS.border}`, 
                  borderRadius: '8px', 
                  padding: '1rem',
                  marginBottom: '1rem',
                }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>
                    SKILL EVOLUTION METRICS
                    {stats.skillEvolutionEnabled && <span style={{ fontSize: '0.65rem', color: COLORS.success, marginLeft: '0.5rem' }}>(Active)</span>}
                  </h4>
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '0.75rem' }}>
                    {[
                      { label: 'Total Skill Updates', value: stats.totalSkillUpdates || 0, color: COLORS.primary },
                      { label: 'Skill Update Rate', value: stats.totalMatches ? `${((stats.totalSkillUpdates || 0) / stats.totalMatches).toFixed(2)}` : '0.00', color: COLORS.tertiary, sub: 'updates per match' },
                      { label: 'Evolution Mode', value: stats.skillEvolutionEnabled ? 'Evolving' : 'Static', color: stats.skillEvolutionEnabled ? COLORS.success : COLORS.textMuted },
                    ].map(({ label, value, color, sub }) => (
                      <div key={label} style={{
                        background: COLORS.darker,
                        border: `1px solid ${COLORS.border}`,
                        borderRadius: '6px',
                        padding: '0.75rem',
                      }}>
                        <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginBottom: '0.25rem' }}>{label}</div>
                        <div style={{ fontSize: '1.1rem', fontWeight: 600, color }}>{value}</div>
                        {sub && <div style={{ fontSize: '0.55rem', color: COLORS.textMuted, marginTop: '0.25rem' }}>{sub}</div>}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Search Queue Visualization */}
              {(() => {
                let searchQueue = [];
                if (sim && wasmReady) {
                  try {
                    const queueJson = sim.get_search_queue();
                    searchQueue = JSON.parse(queueJson);
                  } catch (error) {
                    console.error('Error getting search queue:', error);
                  }
                }
                const partySearches = searchQueue.filter(s => s.is_party);
                const soloSearches = searchQueue.filter(s => !s.is_party);

                return (
                  <div style={{ 
                    background: COLORS.card, 
                    border: `1px solid ${COLORS.border}`, 
                    borderRadius: '8px', 
                    padding: '1rem',
                    marginBottom: '1rem',
                  }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.75rem', letterSpacing: '0.1em' }}>SEARCH QUEUE</h4>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem', marginBottom: '0.75rem' }}>
                      <div style={{ background: COLORS.darker, padding: '0.5rem', borderRadius: '4px' }}>
                        <div style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>Party Searches</div>
                        <div style={{ fontSize: '1.25rem', fontWeight: 600, color: COLORS.success }}>{partySearches.length}</div>
                        {partySearches.length > 0 && (
                          <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginTop: '0.25rem' }}>
                            Avg size: {(partySearches.reduce((sum, s) => sum + s.size, 0) / partySearches.length).toFixed(1)}
                          </div>
                        )}
                      </div>
                      <div style={{ background: COLORS.darker, padding: '0.5rem', borderRadius: '4px' }}>
                        <div style={{ fontSize: '0.65rem', color: COLORS.textMuted }}>Solo Searches</div>
                        <div style={{ fontSize: '1.25rem', fontWeight: 600, color: COLORS.warning }}>{soloSearches.length}</div>
                      </div>
                    </div>
                    {searchQueue.length > 0 && (
                      <div style={{ maxHeight: '150px', overflowY: 'auto' }}>
                        {searchQueue.slice(0, 10).map(search => (
                          <div
                            key={search.id}
                            style={{
                              padding: '0.5rem',
                              marginBottom: '0.25rem',
                              background: COLORS.darker,
                              border: `1px solid ${search.is_party ? COLORS.success : COLORS.border}`,
                              borderRadius: '4px',
                              fontSize: '0.65rem',
                              display: 'flex',
                              justifyContent: 'space-between',
                              alignItems: 'center',
                            }}
                          >
                            <div>
                              <span style={{ 
                                color: search.is_party ? COLORS.success : COLORS.textMuted,
                                fontWeight: 600,
                                marginRight: '0.5rem',
                              }}>
                                {search.is_party ? '👥 Party' : '👤 Solo'}
                              </span>
                              <span style={{ color: COLORS.text }}>Size: {search.size}</span>
                            </div>
                            <div style={{ color: COLORS.textMuted }}>
                              {search.wait_time.toFixed(1)}s
                            </div>
                          </div>
                        ))}
                        {searchQueue.length > 10 && (
                          <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, textAlign: 'center', padding: '0.25rem' }}>
                            +{searchQueue.length - 10} more searches
                          </div>
                        )}
                      </div>
                    )}
                    {searchQueue.length === 0 && (
                      <div style={{ fontSize: '0.65rem', color: COLORS.textMuted, textAlign: 'center', padding: '0.5rem' }}>
                        No active searches
                      </div>
                    )}
                  </div>
                );
              })()}

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
            <div>
              {/* Party Visualizations */}
              {parties.length > 0 && (
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem', marginBottom: '0.75rem' }}>
                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PARTY SIZE DISTRIBUTION</h4>
                    <ResponsiveContainer width="100%" height={200}>
                      <BarChart data={(() => {
                        const sizeCounts = {};
                        parties.forEach(p => {
                          sizeCounts[p.size] = (sizeCounts[p.size] || 0) + 1;
                        });
                        return Object.entries(sizeCounts).map(([size, count]) => ({
                          size: `${size} players`,
                          count,
                        })).sort((a, b) => {
                          const aSize = parseInt(a.size);
                          const bSize = parseInt(b.size);
                          return aSize - bSize;
                        });
                      })()}>
                        <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                        <XAxis dataKey="size" tick={{ fill: COLORS.textMuted, fontSize: 9 }} />
                        <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                        <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                        <Bar dataKey="count" fill={COLORS.primary} radius={[2, 2, 0, 0]} />
                      </BarChart>
                    </ResponsiveContainer>
                  </div>

                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PARTY VS SOLO SEARCH TIMES</h4>
                    <ResponsiveContainer width="100%" height={200}>
                      <BarChart data={[
                        { type: 'Party', avg: stats.partySearchTimes && stats.partySearchTimes.length > 0 
                          ? stats.partySearchTimes.reduce((a, b) => a + b, 0) / stats.partySearchTimes.length
                          : 0 },
                        { type: 'Solo', avg: stats.soloSearchTimes && stats.soloSearchTimes.length > 0
                          ? stats.soloSearchTimes.reduce((a, b) => a + b, 0) / stats.soloSearchTimes.length
                          : 0 },
                      ]}>
                        <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                        <XAxis dataKey="type" tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                        <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Avg Search Time (s)', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }} />
                        <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} formatter={(v) => `${Number(v).toFixed(1)}s`} />
                        <Bar dataKey="avg" radius={[2, 2, 0, 0]}>
                          {[COLORS.success, COLORS.warning].map((color, i) => (
                            <Cell key={i} fill={color} />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                </div>
              )}

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

              {/* New Slice C Charts */}
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem', marginTop: '0.75rem' }}>
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>BLOWOUT RATE BY PLAYLIST</h4>
                  <ResponsiveContainer width="100%" height={250}>
                    <BarChart data={(() => {
                      if (!stats.perPlaylistBlowoutRate) return [];
                      return Object.entries(stats.perPlaylistBlowoutRate).map(([playlist, rate]) => ({
                        playlist: playlist.replace(/([A-Z])/g, ' $1').trim(),
                        rate: rate * 100,
                      }));
                    })()}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="playlist" tick={{ fill: COLORS.textMuted, fontSize: 9 }} angle={-45} textAnchor="end" height={60} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Blowout Rate (%)', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} formatter={(v) => `${Number(v).toFixed(1)}%`} />
                      <Bar dataKey="rate" fill={COLORS.danger} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>

                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>BLOWOUT SEVERITY DISTRIBUTION</h4>
                  <ResponsiveContainer width="100%" height={250}>
                    <BarChart data={(() => {
                      if (!stats.blowoutSeverityCounts) return [];
                      const severityMap = { 'Mild': 0, 'Moderate': 0, 'Severe': 0 };
                      Object.entries(stats.blowoutSeverityCounts).forEach(([severity, count]) => {
                        severityMap[severity] = count;
                      });
                      return Object.entries(severityMap).map(([severity, count]) => ({ severity, count }));
                    })()}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="severity" tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Bar dataKey="count" radius={[2, 2, 0, 0]}>
                        {[COLORS.warning, COLORS.quaternary, COLORS.danger].map((color, i) => (
                          <Cell key={i} fill={color} />
                        ))}
                      </Bar>
                    </BarChart>
                  </ResponsiveContainer>
                </div>

                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>TEAM SKILL DIFFERENCE DISTRIBUTION</h4>
                  <ResponsiveContainer width="100%" height={250}>
                    <BarChart data={(() => {
                      if (!stats.teamSkillDifferenceSamples || stats.teamSkillDifferenceSamples.length === 0) return [];
                      const samples = stats.teamSkillDifferenceSamples;
                      const max = Math.max(...samples);
                      const bins = 20;
                      const binWidth = max / bins;
                      const histogram = Array(bins).fill(0).map((_, i) => ({
                        range: `${(i * binWidth).toFixed(2)}-${((i + 1) * binWidth).toFixed(2)}`,
                        count: 0,
                      }));
                      samples.forEach(sample => {
                        const bin = Math.min(Math.floor(sample / binWidth), bins - 1);
                        histogram[bin].count += 1;
                      });
                      return histogram;
                    })()}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="range" tick={{ fill: COLORS.textMuted, fontSize: 8 }} angle={-45} textAnchor="end" height={60} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Bar dataKey="count" fill={COLORS.primary} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </div>
              
              {/* Slice E: Retention Model Charts */}
              <div style={{ marginTop: '0.75rem' }}>
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginBottom: '0.75rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>CONTINUATION RATE BY SKILL BUCKET</h4>
                  <ResponsiveContainer width="100%" height={300}>
                    <BarChart data={(() => {
                      if (!retentionStats?.per_bucket_continue_rate) return [];
                      return Object.entries(retentionStats.per_bucket_continue_rate).map(([bucket, rate]) => ({
                        bucket: parseInt(bucket),
                        rate: rate,
                        ratePercent: (rate * 100).toFixed(1),
                      })).sort((a, b) => a.bucket - b.bucket);
                    })()}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Skill Bucket', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} domain={[0, 1]} tickFormatter={(v) => `${(v*100).toFixed(0)}%`} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} formatter={(v) => `${(v*100).toFixed(1)}%`} />
                      <Bar dataKey="rate" fill={COLORS.primary} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
                
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>SESSION LENGTH DISTRIBUTION</h4>
                    <ResponsiveContainer width="100%" height={250}>
                      <BarChart data={(() => {
                        if (!sessionStats?.session_length_distribution) return [];
                        return sessionStats.session_length_distribution.map((count, matches) => ({
                          matches,
                          count,
                        })).filter(d => d.count > 0).slice(0, 20);
                      })()}>
                        <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                        <XAxis dataKey="matches" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Matches per Session', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                        <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} />
                        <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                        <Bar dataKey="count" fill={COLORS.secondary} radius={[2, 2, 0, 0]} />
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                  
                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>RETENTION METRICS</h4>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text }}>
                        Avg Matches/Session: <span style={{ color: COLORS.primary }}>{sessionStats?.avg_matches_per_session?.toFixed(2) ?? '0.00'}</span>
                      </div>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text }}>
                        Active Sessions: <span style={{ color: COLORS.primary }}>{retentionStats?.active_sessions ?? 0}</span>
                      </div>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text }}>
                        Total Sessions: <span style={{ color: COLORS.primary }}>{sessionStats?.total_sessions_completed ?? 0}</span>
                      </div>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text }}>
                        Population Change: <span style={{ color: ((stats?.populationChangeRate || 0) >= 0 ? COLORS.success : COLORS.danger) }}>
                          {((stats?.populationChangeRate || 0) >= 0 ? '+' : '')}{(stats?.populationChangeRate || 0).toFixed(2)}/s
                        </span>
                        <span style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginLeft: '0.25rem' }}>
                          ({((stats?.populationChangeRate || 0) >= 0 ? 'growing' : 'shrinking')})
                        </span>
                        {retentionStats?.recent_population_samples && retentionStats.recent_population_samples.length > 0 && (
                          <div style={{ fontSize: '0.55rem', color: COLORS.textMuted, marginTop: '0.15rem', fontStyle: 'italic' }}>
                            Recent: {retentionStats.recent_population_samples.slice(-5).join(', ')}
                          </div>
                        )}
                      </div>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text }}>
                        Total Returns: <span style={{ color: COLORS.primary }}>{returnStats?.total_returns ?? 0}</span>
                      </div>
                      <div style={{ fontSize: '0.7rem', color: COLORS.text, marginTop: '0.5rem', paddingTop: '0.5rem', borderTop: `2px solid ${COLORS.quaternary}`, backgroundColor: `${COLORS.darker}40` }}>
                        <div style={{ fontSize: '0.7rem', color: COLORS.quaternary, fontWeight: 600, marginBottom: '0.25rem' }}>🔍 DIAGNOSTIC DATA</div>
                        <div style={{ fontSize: '0.65rem', color: COLORS.text, marginBottom: '0.15rem' }}>
                          Avg Continue Prob: <span style={{ color: COLORS.quaternary, fontWeight: 600 }}>{(retentionStats?.avg_computed_continue_prob ?? 0).toFixed(3)}</span>
                          <span style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginLeft: '0.25rem' }}>
                            ({(retentionStats?.avg_computed_continue_prob ?? 0) * 100}% continue, {(1 - (retentionStats?.avg_computed_continue_prob ?? 0)) * 100}% quit)
                          </span>
                        </div>
                        {retentionStats?.sample_logits && retentionStats.sample_logits.length > 0 ? (
                          <>
                            <div style={{ fontSize: '0.65rem', color: COLORS.text, marginBottom: '0.15rem' }}>
                              Avg Logit: <span style={{ color: COLORS.primary }}>{(retentionStats.sample_logits.reduce((a, b) => a + b, 0) / retentionStats.sample_logits.length).toFixed(2)}</span>
                            </div>
                            {retentionStats?.sample_experiences && retentionStats.sample_experiences.length > 0 && (
                              <div style={{ fontSize: '0.65rem', color: COLORS.text, marginBottom: '0.15rem' }}>
                                Avg Experience: 
                                <span style={{ marginLeft: '0.25rem' }}>
                                  δP=<span style={{ color: COLORS.primary }}>{(retentionStats.sample_experiences.reduce((a, b) => a + b[0], 0) / retentionStats.sample_experiences.length).toFixed(1)}</span>ms,
                                  ST=<span style={{ color: COLORS.primary }}>{(retentionStats.sample_experiences.reduce((a, b) => a + b[1], 0) / retentionStats.sample_experiences.length).toFixed(1)}</span>s,
                                  WR=<span style={{ color: COLORS.primary }}>{(retentionStats.sample_experiences.reduce((a, b) => a + b[3], 0) / retentionStats.sample_experiences.length * 100).toFixed(0)}</span>%,
                                  Perf=<span style={{ color: COLORS.primary }}>{(retentionStats.sample_experiences.reduce((a, b) => a + b[4], 0) / retentionStats.sample_experiences.length).toFixed(2)}</span>
                                </span>
                              </div>
                            )}
                            {retentionStats?.current_retention_config && (
                              <div style={{ fontSize: '0.65rem', color: COLORS.text, marginTop: '0.25rem', paddingTop: '0.25rem', borderTop: `1px solid ${COLORS.border}` }}>
                                <div style={{ fontWeight: 600, marginBottom: '0.1rem' }}>Active Config:</div>
                                <div style={{ fontSize: '0.6rem', color: COLORS.textMuted }}>
                                  θPing: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.theta_ping?.toFixed(3) ?? 'N/A'}</span> | 
                                  θSearchTime: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.theta_search_time?.toFixed(3) ?? 'N/A'}</span> | 
                                  θBlowout: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.theta_blowout?.toFixed(3) ?? 'N/A'}</span>
                                </div>
                                <div style={{ fontSize: '0.6rem', color: COLORS.textMuted }}>
                                  θWinRate: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.theta_win_rate?.toFixed(3) ?? 'N/A'}</span> | 
                                  θPerf: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.theta_performance?.toFixed(3) ?? 'N/A'}</span> | 
                                  Base: <span style={{ color: COLORS.primary }}>{retentionStats.current_retention_config.base_continue_prob?.toFixed(3) ?? 'N/A'}</span>
                                </div>
                              </div>
                            )}
                          </>
                        ) : (
                          <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, fontStyle: 'italic' }}>
                            Waiting for data... (run simulation to see diagnostic info)
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
                
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginTop: '0.75rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>EFFECTIVE POPULATION SIZE OVER TIME</h4>
                  <ResponsiveContainer width="100%" height={250}>
                    <LineChart data={effectivePopulationHistory}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="tick" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Time (ticks)', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Concurrent Players', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} />
                      <Line type="monotone" dataKey="population" stroke={COLORS.primary} strokeWidth={2} dot={false} />
                    </LineChart>
                  </ResponsiveContainer>
                </div>
                
                <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginTop: '0.75rem' }}>
                  <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>RETURN RATE BY SKILL BUCKET</h4>
                  <ResponsiveContainer width="100%" height={250}>
                    <BarChart data={(() => {
                      if (!returnStats?.per_bucket_return_rate) return [];
                      return Object.entries(returnStats.per_bucket_return_rate).map(([bucket, rate]) => ({
                        bucket: parseInt(bucket),
                        rate: rate,
                        ratePercent: (rate * 100).toFixed(1),
                      })).sort((a, b) => a.bucket - b.bucket);
                    })()}>
                      <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                      <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Skill Bucket', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }} />
                      <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} domain={[0, 1]} tickFormatter={(v) => `${(v*100).toFixed(0)}%`} />
                      <Tooltip contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }} formatter={(v) => `${(v*100).toFixed(1)}%`} />
                      <Bar dataKey="rate" fill={COLORS.quaternary} radius={[2, 2, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </div>
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

              {/* Slice D: Skill Evolution Charts */}
              {config.enableSkillEvolution && (
                <div style={{ marginTop: '0.75rem' }}>
                  <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem', marginBottom: '0.75rem' }}>
                    <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>
                      SKILL EVOLUTION OVER TIME (Mean Skill per Bucket)
                      {stats?.skillEvolutionEnabled && <span style={{ fontSize: '0.65rem', color: COLORS.success, marginLeft: '0.5rem' }}>(Evolving)</span>}
                    </h4>
                    <ResponsiveContainer width="100%" height={400}>
                      <LineChart data={(() => {
                        if (!skillEvolutionData || skillEvolutionData.length === 0) return [];
                        // Use actual snapshot data points
                        const data = skillEvolutionData.map(snapshot => {
                          const point = { tick: snapshot.tick, time: (snapshot.tick * (config.tickInterval || 5)).toFixed(0) };
                          snapshot.buckets.forEach(bucket => {
                            point[`B${bucket.bucket}`] = bucket.mean_skill;
                          });
                          return point;
                        });
                        return data;
                      })()}>
                        <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                        <XAxis 
                          dataKey="tick" 
                          tick={{ fill: COLORS.textMuted, fontSize: 9 }} 
                          label={{ value: 'Simulation Tick', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }}
                          type="number"
                          scale="linear"
                        />
                        <YAxis 
                          tick={{ fill: COLORS.textMuted, fontSize: 10 }} 
                          label={{ value: 'Mean Skill [-1 to 1]', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }}
                          domain={[-1, 1]}
                        />
                        <Tooltip 
                          contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }}
                          formatter={(value, name) => [value?.toFixed(3) || 'N/A', name]}
                          labelFormatter={(label) => `Tick: ${label}`}
                        />
                        <Legend 
                          wrapperStyle={{ fontSize: '0.65rem', color: COLORS.textMuted }}
                          iconType="line"
                        />
                        {/* Show key buckets: Low (1-2), Mid (5-6), High (9-10) */}
                        {[1, 2, 5, 6, 9, 10].map(bucketId => {
                          const colors = [
                            COLORS.danger,    // Bucket 1 (lowest)
                            COLORS.warning,   // Bucket 2
                            COLORS.tertiary,  // Bucket 5
                            COLORS.primary,   // Bucket 6
                            COLORS.success,   // Bucket 9
                            COLORS.quaternary, // Bucket 10 (highest)
                          ];
                          return (
                            <Line 
                              key={bucketId} 
                              type="monotone" 
                              dataKey={`B${bucketId}`} 
                              stroke={colors[bucketId <= 2 ? bucketId - 1 : bucketId <= 6 ? bucketId - 3 : bucketId - 7]} 
                              strokeWidth={2}
                              dot={false}
                              name={`Bucket ${bucketId}${bucketId <= 2 ? ' (Low)' : bucketId >= 9 ? ' (High)' : ' (Mid)'}`}
                              connectNulls
                            />
                          );
                        })}
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                  
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                    <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                      <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>SKILL BY BUCKET (Current)</h4>
                      <ResponsiveContainer width="100%" height={250}>
                        <BarChart data={(() => {
                          if (!skillEvolutionData || skillEvolutionData.length === 0) return [];
                          const latest = skillEvolutionData[skillEvolutionData.length - 1];
                          return latest.buckets.map(b => ({
                            bucket: `B${b.bucket}`,
                            skill: b.mean_skill,
                            label: b.bucket <= 2 ? 'Low' : b.bucket >= 9 ? 'High' : 'Mid',
                          })).sort((a, b) => parseInt(a.bucket.substring(1)) - parseInt(b.bucket.substring(1)));
                        })()}>
                          <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                          <XAxis dataKey="bucket" tick={{ fill: COLORS.textMuted, fontSize: 9 }} />
                          <YAxis 
                            tick={{ fill: COLORS.textMuted, fontSize: 10 }} 
                            domain={[-1, 1]}
                            label={{ value: 'Mean Skill', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }}
                          />
                          <Tooltip 
                            contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }}
                            formatter={(value) => value?.toFixed(3) || 'N/A'}
                          />
                          <Bar dataKey="skill" radius={[2, 2, 0, 0]}>
                            {(() => {
                              if (!skillEvolutionData || skillEvolutionData.length === 0) return null;
                              const latest = skillEvolutionData[skillEvolutionData.length - 1];
                              return latest.buckets.map((b, idx) => {
                                const bucketNum = b.bucket;
                                const color = bucketNum <= 2 ? COLORS.danger : bucketNum >= 9 ? COLORS.success : COLORS.tertiary;
                                return <Cell key={idx} fill={color} />;
                              });
                            })()}
                          </Bar>
                        </BarChart>
                      </ResponsiveContainer>
                    </div>

                    <div style={{ background: COLORS.card, border: `1px solid ${COLORS.border}`, borderRadius: '8px', padding: '1rem' }}>
                      <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.5rem' }}>PERFORMANCE DISTRIBUTION</h4>
                      <ResponsiveContainer width="100%" height={250}>
                        <BarChart data={performanceDistribution}>
                          <CartesianGrid strokeDasharray="3 3" stroke={COLORS.border} />
                          <XAxis 
                            dataKey="bin_start" 
                            tick={{ fill: COLORS.textMuted, fontSize: 9 }} 
                            label={{ value: 'Performance Index', position: 'insideBottom', offset: -5, fill: COLORS.textMuted }}
                            tickFormatter={(v) => v.toFixed(2)}
                          />
                          <YAxis tick={{ fill: COLORS.textMuted, fontSize: 10 }} label={{ value: 'Count', angle: -90, position: 'insideLeft', fill: COLORS.textMuted }} />
                          <Tooltip 
                            contentStyle={{ background: COLORS.card, border: `1px solid ${COLORS.border}` }}
                            formatter={(value, name, props) => [
                              `${value} samples`,
                              `Range: ${props.payload.bin_start.toFixed(2)} - ${props.payload.bin_end.toFixed(2)}`
                            ]}
                          />
                          <Bar dataKey="count" fill={COLORS.tertiary} radius={[2, 2, 0, 0]} />
                        </BarChart>
                      </ResponsiveContainer>
                    </div>
                  </div>
                  
                  {/* Skill Drift Summary */}
                  {skillEvolutionData && skillEvolutionData.length >= 2 && (
                    <div style={{ 
                      background: COLORS.card, 
                      border: `1px solid ${COLORS.border}`, 
                      borderRadius: '8px', 
                      padding: '1rem',
                      marginTop: '0.75rem',
                    }}>
                      <h4 style={{ fontSize: '0.75rem', color: COLORS.textMuted, marginBottom: '0.75rem' }}>SKILL DRIFT SUMMARY</h4>
                      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '0.75rem' }}>
                        {(() => {
                          const first = skillEvolutionData[0];
                          const last = skillEvolutionData[skillEvolutionData.length - 1];
                          const firstMap = new Map(first.buckets.map(b => [b.bucket, b.mean_skill]));
                          const lastMap = new Map(last.buckets.map(b => [b.bucket, b.mean_skill]));
                          
                          // Calculate average skill change
                          let totalChange = 0;
                          let count = 0;
                          firstMap.forEach((firstSkill, bucket) => {
                            const lastSkill = lastMap.get(bucket);
                            if (lastSkill !== undefined) {
                              totalChange += (lastSkill - firstSkill);
                              count++;
                            }
                          });
                          const avgChange = count > 0 ? totalChange / count : 0;
                          
                          // Find buckets with most change
                          const changes = Array.from(firstMap.keys()).map(bucket => ({
                            bucket,
                            change: (lastMap.get(bucket) || 0) - (firstMap.get(bucket) || 0),
                          })).sort((a, b) => Math.abs(b.change) - Math.abs(a.change));
                          
                          return [
                            { 
                              label: 'Avg Skill Change', 
                              value: avgChange > 0 ? `+${avgChange.toFixed(4)}` : avgChange.toFixed(4), 
                              color: avgChange > 0 ? COLORS.success : avgChange < 0 ? COLORS.danger : COLORS.textMuted,
                              sub: 'Overall drift'
                            },
                            { 
                              label: 'Most Improved', 
                              value: `B${changes[0]?.bucket || 'N/A'}`, 
                              color: COLORS.success,
                              sub: changes[0] ? `+${changes[0].change.toFixed(3)}` : 'N/A'
                            },
                            { 
                              label: 'Most Declined', 
                              value: `B${changes[changes.length - 1]?.bucket || 'N/A'}`, 
                              color: COLORS.danger,
                              sub: changes[changes.length - 1] ? `${changes[changes.length - 1].change.toFixed(3)}` : 'N/A'
                            },
                          ].map(({ label, value, color, sub }) => (
                            <div key={label} style={{
                              background: COLORS.darker,
                              border: `1px solid ${COLORS.border}`,
                              borderRadius: '6px',
                              padding: '0.75rem',
                            }}>
                              <div style={{ fontSize: '0.6rem', color: COLORS.textMuted, marginBottom: '0.25rem' }}>{label}</div>
                              <div style={{ fontSize: '1.1rem', fontWeight: 600, color }}>{value}</div>
                              {sub && <div style={{ fontSize: '0.55rem', color: COLORS.textMuted, marginTop: '0.25rem' }}>{sub}</div>}
                            </div>
                          ));
                        })()}
                      </div>
                    </div>
                  )}
                </div>
              )}
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
