
# Designing a Matchmaking Model for a Call of Duty–Style System

Let’s treat this like designing a physics model of a big messy system:

1. First we decide **what the state of the universe is** (all variables).
2. Then we define **what rules move it forward in time**.
3. Finally, we decide **where we can safely approximate** without losing the behaviors we care about.

This document covers:

- What we want out of the model  
- A full “high‑fidelity” mathematical model (agent‑based, all variables)  
- Reduced / aggregate models that scale to “millions” and are fast  
- Where to simplify each of the CoD variables  
- A concrete build order: how to construct this as a research simulation  

The focus is math‑y but not code‑y.

---

## 1. What are we modeling, exactly?

We want a model that, given:

- A population of players (skill distribution, geography, platforms, preferences)
- Data centers (DCs) and their capacities
- Matchmaking rules (ping backoffs, skill constraints, mode/playlist constraints)

can produce, over time:

- Search time distributions (by region, playlist, skill bucket)
- Delta ping distributions
- Skill disparity in lobbies
- Outcome metrics (win rate vs skill, blowouts, KPM/SPM distributions)
- Behavioral KPIs (quit rates, retention/churn, etc.)

So we need:

- A **stochastic dynamic model** (continuous time, discrete events)
- Where players are agents in a loop:

  ```text
  search → match → play → outcome → search/quit
  ```

- And matchmaking is an algorithmic operator that maps “searching agents” to “matches” subject to constraints.

---

## 2. State & variables in a full high‑fidelity model

### 2.1 Sets and indices

- Players:  
  \( i = 1, \dots, N \)

- Data centers:  
  \( d \in \mathcal{D} \)

- Playlists (modes):  
  \( m \in \mathcal{M} \)  
  (e.g. TDM, S&D, Ground War, etc.)

- Skill buckets (for analysis):  
  \( b \in \{1,\dots,B\} \) (deciles, vigintiles, etc.)

- Time is continuous:  
  \( t \ge 0 \).

---

### 2.2 Player‑level static attributes

For each player \( i \):

- **Location / region** (e.g. lat/long):  
  \( \mathbf{r}_i \in \mathbb{R}^2 \)

- **Platform** (PC, Xbox, PS, etc.):  
  \( P_i \in \mathcal{P} \)

- **Input device**:  
  \( I_i \in \{\text{controller}, \text{mkb}\} \)

- **Voice chat** enabled/disabled:  
  \( V_i \in \{0,1\} \)

- **Playlist preference vector**:  
  \( \mathbf{w}_i \in \Delta^{|\mathcal{M}|-1} \)  
  where \( w_{i,m} \) is the probability (or weight) that player \( i \) wants playlist \( m \).

- **Quick Play behavior** can be modeled as a set of eligible playlists:  
  \( S_i(t) \subseteq \mathcal{M} \).

---

### 2.3 Player–DC connectivity and ping

Each client regularly measures ping to all data centers. For each player–DC pair:

- **Base ping (round‑trip latency)**:  
  \( p_{i,d} \in \mathbb{R}_+ \).

- **Best data center and its ping**:

  \[
  d_i^\* = \arg\min_d p_{i,d}, \quad
  p_i^\* = \min_d p_{i,d}.
  \]

- **Delta ping** for a given match DC \( \hat d \):

  \[
  \Delta p_i = p_{i,\hat d} - p_i^\*.
  \]

We don’t necessarily need the full \( N \times |\mathcal{D}| \) matrix in a reduced model. It can be enough to know, for each player, their ranked list of DCs and pings:

- \( p_{i,(1)}, p_{i,(2)}, \dots \) with \( p_{i,(1)} = p_i^\* \).

For a search started at time \( \tau \), define the **acceptable DC set** as a function of waiting time \( w \):

\[
\mathcal{D}_i(w) =
\{ d : p_{i,d} \le p_i^\* + f_{\text{conn}}(w) \ \text{and}\ p_{i,d} \le P_{\max} \}.
\]

- \( f_{\text{conn}}(w) \): delta‑ping backoff curve (increasing with search time)  
- \( P_{\max} \): hard maximum ping (varies by region/mode)

---

### 2.4 Skill model

We assume each player has:

- A **raw skill** value (bounded, continuous).  
- A **skill percentile** (0–1) for bucketization.

For each player \( i \) at time \( t \):

- Raw skill (e.g. in \([-1,1]\)):  
  \( s_i(t) \in [-1,1] \).

- Skill percentile:  
  \( \pi_i(t) \in [0,1] \).

- Skill bucket:  
  \[
  b_i(t) = \left\lfloor B \cdot \pi_i(t) \right\rfloor + 1.
  \]

**Party‑level aggregates.** For a party \( P \subseteq \{1,\dots,N\} \):

\[
\bar{s}_P = \frac{1}{|P|}\sum_{i\in P} s_i, \quad
\Delta s_P = \max_{i\in P} s_i - \min_{i\in P} s_i.
\]

Similarly in percentile space:

\[
\bar{\pi}_P, \quad \Delta \pi_P.
\]

These are used for skill similarity and disparity rules.

---

### 2.5 Player activity state machine

At any time \( t \), player \( i \) is in one of several states:

- **OFFLINE**: \( X_i(t) = \text{off} \)
- **IN_LOBBY** (menus)
- **SEARCHING**: \( X_i(t) = \text{search} \)
- **IN_MATCH**: \( X_i(t) = \text{match} \)

**Session arrivals** can be modeled as a non‑homogeneous Poisson process with rate \( \lambda_i(t) \):

\[
\mathbb{P}(\text{player } i \text{ starts a session in } [t, t+dt)) \approx \lambda_i(t)\,dt.
\]

Within a session, the core loop is:

```text
search → matched → play match (duration L) → outcome → decide: search again or quit
```

Let:

- \( L_{i,k} \) = length of the \(k\)-th match of player \( i \) (depends on playlist).  
- \( \rho_i \) = probability of searching for another match after one ends (can depend on recent experiences).

---

### 2.6 Playlists, match sizes, and servers

For each playlist \( m \):

- Required lobby size: \( N^{\text{req}}_m \) (e.g. 12 for 6v6 TDM)
- Teams (2‑team, 3‑squad, etc.)
- Typical match length distribution: \( L_m \)
- Backoff parameters:
  - \( f_{\text{conn},m}(w) \): delta ping backoff curve
  - \( f_{\text{skill},m}(w) \): skill similarity/disparity backoff curve

For each data center \( d \) and playlist \( m \):

- Number of servers: \( K_{d,m}(t) \)
- Busy servers: \( B_{d,m}(t) \)
- Free servers: \( F_{d,m}(t) = K_{d,m}(t) - B_{d,m}(t) \)

Server capacity interacts with DC choice: if the best DC has no free server, the matchmaker may choose another acceptable DC.

---

### 2.7 Search objects (what the matchmaker sees)

The matchmaker operates on **search objects**, which are either parties or incomplete lobbies.

Let \( j \in S(t) \) index search objects at time \( t \). Each search \( j \) has:

- Player set: \( P_j \subseteq \{1,\dots,N\} \)
- Size: \( n_j = |P_j| \)
- Average skill & disparity: \( \bar{\pi}_j, \Delta\pi_j \)
- Location (e.g. average coordinates): \( \mathbf{r}_j \)
- Platform composition: \( P_j^{\text{plat}} \)
- Input device composition: \( I_j^{\text{input}} \)
- Voice chat flags
- Playlist preferences: \( S_j \subseteq \mathcal{M} \)
- Search start time: \( \tau_j \)

For each player \( i \in P_j \), define acceptable DC set \( \mathcal{D}_i(w_j) \) where \( w_j = t - \tau_j \). The search‑level acceptable DC set is:

\[
\mathcal{D}_j(t) = \bigcap_{i\in P_j} \mathcal{D}_i(t - \tau_j).
\]

Skill constraints: each search has an acceptable lobby skill range that widens over time:

\[
[\ell_j(t), u_j(t)] = [\bar{\pi}_j - f_{\text{skill},m}(w_j),\, \bar{\pi}_j + f_{\text{skill},m}(w_j)].
\]

Maximum acceptable lobby disparity:

\[
\Delta\pi^{\max}_j(t).
\]

These implement skill similarity and disparity rules.

---

## 3. High‑fidelity matchmaking process

At a high level, every \( \Delta T \) seconds (e.g. 5s), the system runs a **matchmaking tick**.

### 3.1 Candidate distance between searches

Define a distance \( D(j,k) \) between two searches \( j, k \):

\[
D(j,k) =
\alpha_{\text{geo}} d_{\text{geo}}(j,k)
+ \alpha_{\text{skill}} d_{\text{skill}}(j,k)
+ \alpha_{\text{input}} d_{\text{input}}(j,k)
+ \alpha_{\text{platform}} d_{\text{plat}}(j,k).
\]

Where:

- \( d_{\text{geo}} \) = great‑circle distance between \( \mathbf{r}_j, \mathbf{r}_k \)
- \( d_{\text{skill}} = |\bar{\pi}_j - \bar{\pi}_k| \)
- \( d_{\text{input}} \) = penalty for mixing input schemes
- \( d_{\text{plat}} \) = cross‑platform penalty

For each seed search \( s \), we sort other searches by \( D(s,\cdot) \) and take the top \( K \) as candidate neighbors.

---

### 3.2 Seed selection

Let \( S(t) \) be the set of current searches. Choose a subset:

\[
S_{\text{seed}}(t) \subseteq S(t)
\]

as seeds (possibly all, or a random subset). For each seed \( s \):

- Candidate set: \( C_s(t) \) = top‑\(K\) neighbors by \( D(s,\cdot) \).

---

### 3.3 Feasibility constraints for forming a match

A match \( M \subseteq S(t) \) for playlist \( m \) is feasible if:

1. **Playlist compatibility**:

   \[
   m \in \bigcap_{j\in M} S_j.
   \]

2. **Lobby size constraint**:

   \[
   \sum_{j\in M} n_j = N^{\text{req}}_m.
   \]

3. **Skill similarity**:

   Let

   \[
   \pi_{\min}(M) = \min_{j\in M} \bar{\pi}_j, \quad
   \pi_{\max}(M) = \max_{j\in M} \bar{\pi}_j.
   \]

   For all \( j \in M \):

   \[
   [\pi_{\min}(M), \pi_{\max}(M)] \subseteq [\ell_j(t), u_j(t)].
   \]

4. **Skill disparity**:

   Define lobby skill disparity:

   \[
   \Delta\pi_M = \pi_{\max}(M) - \pi_{\min}(M).
   \]

   For all \( j \in M \):

   \[
   \Delta\pi_M \le \Delta\pi^{\max}_j(t).
   \]

5. **Data center intersection**:

   \[
   \mathcal{D}_M(t) = \bigcap_{j\in M} \mathcal{D}_j(t)
   \]

   must be non‑empty.

6. **Server capacity**:

   There exists \( d \in \mathcal{D}_M(t) \) such that \( F_{d,m}(t) \ge 1 \).

---

### 3.4 Quality score for candidate matches

Among feasible \( M \), choose those maximizing a **quality score** \( Q(M) \). Components might include:

- Ping quality \( Q_{\text{ping}}(M) \) (inverse of average delta ping)
- Skill balance \( Q_{\text{skill\_balance}}(M) \)
- Search time fairness \( Q_{\text{wait\_time}}(M) \)
- Playlist/map diversity \( Q_{\text{diversity}}(M) \)

A generic form:

\[
Q(M) =
\beta_1 Q_{\text{ping}}(M)
+ \beta_2 Q_{\text{skill\_balance}}(M)
+ \beta_3 Q_{\text{wait\_time}}(M)
+ \beta_4 Q_{\text{diversity}}(M).
\]

Weights \( \beta \) are tunable parameters in the model.

---

### 3.5 Greedy match construction

For each seed \( s \):

1. Initialize \( M = \{s\} \).
2. Traverse candidates \( j \in C_s(t) \) in order of increasing \( D(s,j) \).
3. For each \( j \):
   - Tentatively set \( M' = M \cup \{j\} \).
   - If constraints (1–6) are satisfied and \( Q(M') \ge Q(M) \), accept \( j \) (set \( M \leftarrow M' \)).
4. Stop when the lobby size is satisfied or no more feasible candidates exist.
5. Commit the match \( M \) and remove its searches from \( S(t) \).

This approximates the seed + greedy heuristic described in the whitepapers.

---

### 3.6 Team balancing inside a lobby

Once a lobby \( M \) is formed with parties \( P_1,\dots,P_K \) of sizes \( n_k = |P_k| \) and average skills \( \bar{s}_k \):

1. Check **balanceability** of party sizes using a Karmarkar–Karp style heuristic (multiway partitioning).
2. Enumerate possible team partitions (for small modes like 6v6):
   - Filter to partitions with minimal team size difference.
   - Among these, choose the one with minimal skill difference:

   \[
   \Delta s_{\text{teams}} =
   \left|
   \sum_{k\in T_1} n_k \bar{s}_k -
   \sum_{k\in T_2} n_k \bar{s}_k
   \right|.
   \]

In the simulation, you can view \( \Delta s_{\text{teams}} \) as a random variable derived from the team‑balancing procedure.

---

### 3.7 Match outcome and skill update

Given teams A and B with total skills \( S_A, S_B \) and team sizes \( n_A, n_B \), define, for example, the win probability:

\[
\mathbb{P}(\text{A wins}) = \sigma\big(\gamma (S_A - S_B)\big)
\]

where \( \sigma \) is a logistic function.

For individual performance (KPM, SPM), you might use:

\[
Y_i = f_{\text{perf}}(s_i, \bar{s}_{\text{lobby}}, m) + \epsilon_i
\]

where:

- \( f_{\text{perf}} \) increases with \( s_i \)
- \( \epsilon_i \) is noise

Skill update (conceptually):

\[
s_i(t^+) = s_i(t^-) + \alpha \left(\hat{y}_i - \mathbb{E}[Y_i \mid s_i, \text{lobby}]\right)
\]

where:

- \( \hat{y}_i \) = normalized performance vs lobby
- \( \alpha \) = learning rate

Exact formulas are flexible; the key is that skill is predictive, summable, and resilient.

---

### 3.8 Player satisfaction and churn

For each match and player, define an **experience vector**:

\[
\mathbf{z}_i = (\Delta p_i,\ T^{\text{search}}_i,\ \text{blowout flag},\ \text{KPM}_i,\ \text{placement percentile}, \dots)
\]

Then define models for:

- **Quit mid‑match**:
  \[
  \mathbb{P}(\text{quit mid‑match} \mid \mathbf{z}_i) = \sigma(\theta^\top \mathbf{z}_i)
  \]

- **Play another match**:
  \[
  \mathbb{P}(\text{play another match} \mid \mathbf{z}_i^{\text{history}}) = \sigma(\phi^\top \bar{\mathbf{z}}_i)
  \]

Parameters \( \theta, \phi \) can be hand‑tuned or fit from data. This connects matchmaking behavior to KPIs like hours‑per‑user and churn.

---

## 4. Why the full model might be too slow

If we literally simulate every player and run the full greedy algorithm every few seconds, performance might be an issue when:

- Simulating **long time horizons** (months of operation)
- Exploring **many parameter settings** (backoff curves, skill weights, etc.)

However, note the important scale observation:

- 1M online players → ~800k in the core loop  
- Match takes 15 minutes, matchmaking takes 15 seconds → ~1/60 of core‑loop players are searching → ~13k searching at once.

13k search objects with top‑\(K\) neighbor search is manageable in an efficient implementation. Still, to run huge scenario sweeps, we want **reduced / aggregate models**.

---

## 5. Reduced / aggregate model for “millions of players”

Instead of tracking every player, we track **counts in buckets**, and approximate the matchmaker as a **flow** between buckets.

### 5.1 Buckets / macro‑state

Define buckets by:

- Region or DC cluster: \( r \in \mathcal{R} \)
- Playlist: \( m \in \mathcal{M} \)
- Skill bucket: \( b \in \{1,\dots,B\} \)
- Waiting‑time class: \( k \in \{1,\dots,K_w\} \) (discretize wait \( w \))

Let:

- \( S_{rmbk}(t) \) = expected number of **searching** players in bucket \((r,m,b,k)\).  
- \( P_{rmb}(t) \) = expected number of players **in matches**.  
- \( H_{rmb}(t) \) = expected number of **idle** players (in menus, ready to search).

We also track DC capacity:

- \( F_{d,m}(t) \) = free servers at DC \( d \), playlist \( m \).

---

### 5.2 Fluid / mean‑field dynamics

Players arrive to search from:

- Idle menus at rate \( \lambda_{rmb}(t) \).  
- Completed matches: from \( P_{rmb}(t) \) at rate \( 1 / \mathbb{E}[L_m] \), times continuation probability \( \rho_{rmb} \).

We model time evolution via ODEs. For example, for the first waiting‑time bin:

\[
\frac{d S_{rmb1}}{dt}
= \lambda_{rmb}(t)
+ \rho_{rmb} \frac{P_{rmb}(t)}{\mathbb{E}[L_m]}
- \mu_{rmb1}(t)
- \text{aging to bin 2},
\]

and for \( k > 1 \):

\[
\frac{d S_{rmbk}}{dt}
= \text{aging from bin }(k-1)
- \mu_{rmbk}(t)
- \text{aging to bin }(k+1).
\]

Players in matches:

\[
\frac{d P_{rmb}}{dt}
= \sum_k \mu_{rmbk}(t)
- \frac{P_{rmb}(t)}{\mathbb{E}[L_m]}.
\]

Here, \( \mu_{rmbk}(t) \) is the **matchmaking throughput** from bucket \((r,m,b,k)\): number of players per unit time who leave searching and enter matches.

Aging moves mass between waiting‑time bins; it approximates the wait‑time distribution.

---

### 5.3 Approximating matchmaking throughput \( \mu \)

We approximate how quickly each bucket is matched and with whom.

For each region \( r \) and playlist \( m \):

- Let \( N^{\text{req}}_m \) be the match size.
- Define a **pairing kernel** \( K_{bb'} \): probability that a player in bucket \( b \) matches with players in bucket \( b' \), given constraints.

When constraints are tight:

- \( K_{bb'} \) is mostly near the diagonal (similar skills).

As waiting‑time bins increase (backoff), \( K_{bb'} \) becomes wider (more cross‑skill mixing).

Total searching mass in (r,m):

\[
S_{rm}(t) = \sum_{b,k} S_{rmbk}(t).
\]

Approximate match formation rate in (r,m):

\[
\nu_{rm}(t)
\approx \min\left(
\frac{S_{rm}(t)}{N^{\text{req}}_m},
\ \sum_{d \in \text{region } r} F_{d,m}(t)
\right).
\]

This is matches per unit time, limited by players and servers.

We allocate player slots across buckets using weights that:

- Favor buckets near local skill density
- Favor higher waiting‑time bins (long‑waiting players get priority)
- Respect approximate skill similarity and disparity bounds

One simple approximation:

\[
\mu_{rmbk}(t)
\approx
\nu_{rm}(t)
\cdot
\frac{S_{rmbk}(t) W_{b,k}(t)}{\sum_{b',k'} S_{rmb'k'}(t) W_{b',k'}(t)},
\]

where \( W_{b,k}(t) \) are weights encoding skill and wait‑time priorities.

This continuous approximation preserves:

- Match rates per (region, playlist, skill)
- Rough structure of who matches with whom
- Search‑time distribution by bucket

---

### 5.4 Delta ping and DC choice in aggregate

In aggregate, for each region \( r \):

- We specify distributions of **base ping** \( p^\* \) to nearest DC.
- We specify distributions of ping to nearby DCs.

Given \( f_{\text{conn},m}(w) \), for each wait bin \( k \) we can compute the probability that the acceptable DC set:

- Still only includes best DC (no backoff)
- Includes neighboring DCs

Let \( \eta_{r,m,k}(d) \) = probability a player in bucket \((r,m,k)\) is assigned to DC \( d \). Then expected delta ping in a bucket \((r,m,b,k)\) is:

\[
\mathbb{E}[\Delta p \mid r,m,b,k]
= \sum_d \eta_{r,m,k}(d)\,
\mathbb{E}[p_{i,d} - p^\*_i \mid \text{bucket } (r,b)].
\]

These expectations can be precomputed from assumed network topology and DC layout.

---

### 5.5 Outcomes and skill evolution in aggregate

Track, for each bucket, mean skill and variance:

\[
\mu_{rmb}(t), \quad \sigma^2_{rmb}(t).
\]

When matches fire at rate \( \nu_{rm}(t) \), skill updates can be approximated as a drift term:

\[
\frac{d\mu_{rmb}}{dt}
= \alpha \cdot \Phi(\mu_{rmb}, \text{neighbor buckets}, \dots)
\]

where \( \Phi \) is derived from the micro‑level skill update rule and pairing kernel \( K_{bb'} \).

If we are not deeply interested in long‑term skill evolution, we can treat the skill distribution as stationary and focus on matchmaking behavior given a fixed distribution.

Similarly, we can approximate blowout probability and KPM/SPM distributions per bucket based on:

- Team skill differences
- Performance variance

This is enough to drive retention/quit models on a per‑bucket basis rather than per player.

---

## 6. What to do with each CoD variable

Here’s how to treat each matchmaking variable in both full and reduced models.

### 6.1 Connection (ping & delta ping)

- **Full model:**  
  Model per‑player ping matrix \( p_{i,d} \), best DC, and acceptable DC set \( \mathcal{D}_i(w) \) with backoff function \( f_{\text{conn}}(w) \). Core and should be explicit.

- **Reduced model:**  
  Bucket by region and DC cluster; precompute distributions of \( \Delta p \) as a function of wait time and backoff.

Do **not** drop this – it’s central.

---

### 6.2 Time to match

This is an **emergent variable**, determined by:

- Arrival rates
- DC capacity
- Playlist popularity
- Constraints

Target search times influence backoff curves \( f_{\text{conn}}, f_{\text{skill}} \). You don’t feed search time in as an input; you tune backoffs to achieve desired distributions.

---

### 6.3 Playlist diversity

- How many playlists a player can select at once  
- How often maps/modes repeat

**Full model:**  
Quick Play is simply “searches with a set \( S_j \) of acceptable playlists”. The matchmaker picks a playlist maximizing overlap.

**Reduced model:**  
Often you can collapse to “each player picks a single playlist per match” and ignore map recency unless your research questions are about variety.

Recommendation:  
Include playlist choice explicitly. Treat map diversity as a secondary quality term initially.

---

### 6.4 Skill / performance

Critical if we want to reproduce:

- Blowout distributions
- Skill‑based experiments (loosen/tighten SBMM)
- Retention effects across skill deciles

**Full model:**  
Track individual skills and apply similarity/disparity rules.

**Reduced model:**  
Bucket skill, define pairing kernel \( K_{bb'} \), and a simplified drift for skill evolution.

You *can* ignore skill to focus purely on connection vs search time, but you lose realism around fairness and SBMM effects.

---

### 6.5 Input device

- CoD uses a penalty if input devices differ (controller vs M+K).

**Full model:**  
Include as another dimension in the distance metric \( D(j,k) \).

**Reduced model:**  
Treat as two subpopulations with slightly reduced mixing. Ignore unless researching cross‑input fairness.

---

### 6.6 Platform

Similar to input device.

**Full model:**  
Tag searches with platform and apply constraints/penalties as needed.

**Reduced model:**  
Either ignore or treat as small reduction in mixing probabilities between platforms.

---

### 6.7 Voice chat

A relatively weak signal compared to ping/skill/time.

You can ignore it in early models, or reintroduce as a tag if needed later.

---

### 6.8 Lobby & match fullness

In practice, matches should start near \( N^{\text{req}}_m \).

**Full model:**  
Represent incomplete lobbies explicitly and allow slightly under‑full starts in low‑population situations.

**Reduced model:**  
Assume matches start full; this is usually fine unless modeling extremely sparse populations.

---

### 6.9 Player KPIs (HPU, quits, retention)

These are **outputs**, not inputs.

Represent them via:

- Logistic / probit models tying quits/returns to:
  - \( \Delta p \)
  - Search time
  - Blowouts
  - Performance vs expectation

This lets you reproduce high‑level empirical effects (e.g., deprioritizing skill increases quits for most players).

---

## 7. Concrete build order for the simulation

Here’s a practical roadmap.

### Stage 0: Single‑queue sanity model

Start simple:

- 1 playlist, 1 DC, no skill.
- Players:
  - Arrive at rate \( \lambda \)
  - Play for mean \( L \)
  - Queue to fill matches of size \( N \) on \( c \) servers.

You can derive:

- Mean search time \( T \)
- Fraction of players searching vs playing

Sanity‑check time scales and capacities (validate the “1 in 60 searching” heuristic).

---

### Stage 1: Full agent‑based model for a single region

- 1 or 2 DCs
- A couple of playlists (TDM + one slower mode)
- 10 skill buckets with a simple performance/skill update
- Explicit search objects, seed + greedy algorithm, DC backoff rules

Use this to verify that you can reproduce:

- Search‑time distributions
- Delta ping distributions
- Skill disparity & blowout rates

Experiment with:

- Tightening/loosening skill constraints
- Different backoff curves

This stage is about getting the **core mechanics** right.

---

### Stage 2: Add global regions & DC graph

Introduce:

- Multiple DCs with a latency graph
- Players with origin regions and realistic \( p_{i,d} \) patterns

Implement backoff over DC sets \( \mathcal{D}_i(w) \) so that:

- Short waits: players stay in their best region/DC
- Long waits: they can spill into neighboring regions

You can then examine behaviors like:

- Lower‑pop regions having higher search times or worse delta ping when pushed into other regions
- Different tuning strategies per region

---

### Stage 3: Skill‑aware population health experiments

Use the model to run SBMM‑style experiments:

- **Deprioritize skill:**
  - Loosen similarity/disparity thresholds
  - Back off skill constraints faster

- **Tighten skill:**
  - Narrow acceptable skill ranges
  - Back off more slowly

Measure:

- Per‑decile quit rates, KPM/SPM, placements
- Long‑run population size as quits feed into churn

The model should qualitatively reproduce patterns like:

- Low/mid‑skill players suffering under looser skill
- High‑skill players enjoying short‑term dominance
- Overall population health degrading when skill is too loose

---

### Stage 4: Derive a reduced / aggregate version

Once the agent‑based model is sound:

- Bucket players into \((\text{region}, \text{playlist}, \text{skill})\) lumps.
- Estimate pairing kernel \( K_{bb'} \) and throughput functions \( \mu_{rmbk} \) empirically from the micro‑sim or analytically.
- Replace discrete events by ODEs / fluid equations for \( S_{rmbk}(t), P_{rmb}(t) \).

Validate that the reduced model reproduces, within acceptable error:

- Search‑time distributions
- Delta‑ping distributions
- Blowout rates
- Retention curves

Then you can scale to “millions of players” easily, because complexity depends on the **number of buckets**, not the number of individual players.

---

## 8. TL;DR design philosophy

- Use the **full agent‑based model** to capture the detailed interplay between:
  - DC backoff
  - Skill similarity & disparity rules
  - Playlist popularity
  - Server capacity

- Then derive a **bucketed / mean‑field model** that:
  - Treats players as mass in \((\text{region}, \text{playlist}, \text{skill}, \text{wait})\) buckets
  - Approximates the greedy matchmaker as continuous flows
  - Preserves the emergent metrics:
    - Search time
    - Delta ping
    - Skill matchups
    - Blowouts
  - And runs fast enough to explore many scenarios and parameter choices

This gives you a principled bridge between the **“micro” world of per-player matchmaking** and the **“macro” world of long‑term ecosystem health and tuning.**
