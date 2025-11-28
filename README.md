# COD Matchmaking Simulator

A detailed agent-based matchmaking simulation for Call of Duty-style games, built with Rust (WebAssembly) and React. This tool is designed for research into matchmaking algorithms, SBMM (Skill-Based Match Making), and player experience optimization.

## 🎮 Features

- **Full Agent-Based Simulation**: Simulates individual players with skills, locations, platforms, and preferences
- **Realistic Matchmaking Algorithm**: Implements seed + greedy matching with skill similarity, delta ping backoff, and data center selection
- **10 Global Data Centers**: Realistic geographic distribution with latency modeling
- **Multiple Playlists**: TDM, Search & Destroy, Domination, Ground War, FFA
- **Research Tools**: Parameter sweeps, A/B testing, per-skill-bucket analysis
- **Real-time Visualization**: Live charts for search times, ping distributions, skill matching quality

## 📊 Research Questions This Can Answer

1. How does tightening/loosening SBMM affect search times across skill buckets?
2. What's the tradeoff between ping quality and skill matching?
3. How do backoff curves affect match quality over time?
4. What causes blowouts and how can they be minimized?
5. How does player retention correlate with match quality?

## 🚀 Quick Start (Web Frontend Only)

The easiest way to run the simulator:

```bash
cd web
npm install
npm run dev
```

Then open http://localhost:3000 in your browser.

## 🦀 Building the Rust/WASM Engine (Optional)

For better performance, you can compile the Rust simulation to WebAssembly:

### Prerequisites

1. Install Rust: https://rustup.rs/
2. Add WASM target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
3. Install wasm-pack:
   ```bash
   cargo install wasm-pack
   ```

### Build

```bash
# From the project root (not web/)
wasm-pack build --target web --out-dir web/src/wasm
```

### Integrate with Frontend

After building, update `web/src/MatchmakingSimulator.jsx` to import the WASM module:

```javascript
import init, { SimulationEngine } from './wasm/cod_matchmaking_sim.js';

// In your component:
useEffect(() => {
  init().then(() => {
    const sim = new SimulationEngine(BigInt(Date.now()));
    sim.generate_population(5000);
    // ...
  });
}, []);
```

## 📁 Project Structure

```
cod_matchmaking_project/
├── Cargo.toml              # Rust project configuration
├── src/
│   ├── lib.rs              # WASM bindings and exports
│   ├── types.rs            # Core data structures
│   ├── matchmaker.rs       # Matchmaking algorithm
│   └── simulation.rs       # Simulation engine
├── web/
│   ├── package.json        # Node.js dependencies
│   ├── vite.config.js      # Vite configuration
│   ├── index.html          # Entry HTML
│   └── src/
│       ├── main.jsx        # React entry point
│       └── MatchmakingSimulator.jsx  # Main component
└── README.md
```

## ⚙️ Configuration Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `skillSimilarityInitial` | Initial skill tolerance for matching | 0.05 |
| `skillSimilarityRate` | How fast skill tolerance relaxes | 0.01/s |
| `skillSimilarityMax` | Maximum skill tolerance | 0.5 |
| `deltaPingInitial` | Initial delta ping tolerance (ms) | 10 |
| `deltaPingRate` | How fast ping tolerance relaxes | 2ms/s |
| `deltaPingMax` | Maximum delta ping tolerance | 100ms |
| `weightSkill` | Weight of skill in distance metric | 0.4 |
| `weightGeo` | Weight of geography in distance metric | 0.3 |
| `arrivalRate` | Players coming online per tick | 10 |

## 📈 Key Metrics

- **Search Time**: Time from queue to match (P50, P90, P99)
- **Delta Ping**: Additional latency vs. best data center
- **Skill Disparity**: Spread of skill in a lobby
- **Blowout Rate**: Percentage of heavily unbalanced matches
- **Per-Bucket Stats**: Metrics broken down by skill decile

## 🔬 Running Experiments

### Parameter Sweep

Use the sidebar buttons to run sweeps:
- **Skill Strictness**: Tests SBMM intensity from loose to tight
- **Skill vs Ping Weight**: Tests prioritizing connection vs. fairness

### Custom Experiments

Modify the `runExperiment` function in `MatchmakingSimulator.jsx`:

```javascript
const runExperiment = (paramName, values) => {
  const results = [];
  for (const value of values) {
    const testConfig = { ...config, [paramName]: value };
    const testSim = new SimulationEngine(testConfig, 42);
    testSim.generatePopulation(population);
    for (let i = 0; i < 500; i++) testSim.tick();
    // Collect metrics...
  }
};
```

## 📚 Documentation

- **[Whitepaper](cod_matchmaking_model.md)**: Full mathematical model specification
- **[Implementation Roadmap](COD_MM_ROADMAP.md)**: Detailed plan for completing the whitepaper implementation in vertical slices

### Model Overview

The simulation implements the model from the whitepaper (`cod_matchmaking_model.md`):

**Current Implementation Status**: ~Stage 1-2 (agent-based model with core matchmaking). See `COD_MM_ROADMAP.md` for detailed status and remaining work.

**Key Components**:
- **Player State Machine**: `OFFLINE → IN_LOBBY → SEARCHING → IN_MATCH → (IN_LOBBY | OFFLINE)`
- **Distance Metric**: `D(j,k) = α_geo·d_geo + α_skill·d_skill + α_input·d_input + α_platform·d_platform`
- **Backoff Functions**: `f_conn(w) = min(δ_init + δ_rate·w, δ_max)`, `f_skill(w) = min(σ_init + σ_rate·w, σ_max)`
- **Win Probability**: `P(A wins) = σ(γ·(S_A - S_B))`

**Whitepaper Mapping**:
- Section 2.1-2.7 → `src/types.rs` (state & variables)
- Section 3.1-3.5 → `src/matchmaker.rs` (matchmaking algorithm)
- Section 3.6-3.8 → `src/simulation.rs` (outcomes, retention)
- Section 6.x → Various (treatment of CoD variables)
- Section 7 → `COD_MM_ROADMAP.md` (build order)

## 🤝 Contributing

Feel free to extend the model with:
- Additional playlists/modes
- Party system simulation
- More sophisticated skill evolution
- Regional population dynamics
- Server capacity constraints

## 📄 License

MIT License - Use freely for research and development.
