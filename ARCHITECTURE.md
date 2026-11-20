You are tasked with architecting and advancing Coral, a next-generation Minecraft 1.8.8 / 1.8.9 (Protocol 47) server engine and distributed network ecosystem written in Rust with strategic low-level native integrations. The foundational network stack, chunk packet lightweight bitmask optimizations, dynamic spawn calculation, persistent seed, and basic command/chat routing are already established.


Your objective is to implement the remaining engine subsystems, protocol-level optimizations, and distributed network primitives required for a 100% complete, production-ready, planetary-scale network capable of sustaining 200,000 to 1,000,000+ concurrent players at deterministic 20.0 TPS (50ms tick budget).


Non-Negotiable Performance Targets:


Memory Footprint: ~1.5–3 MB RAM per active player; < 150 MB per loaded minigame world.


CPU Density: 1,500–2,500+ active players per physical CPU core via lock-free, cache-aligned execution.


World Instantiation: < 2ms RAM load times for ephemeral arenas; zero main-thread disk I/O blocking.


Zero-Allocation Hot Paths: Pre-allocated ring buffers, bytes::BytesMut reuse, and zero heap allocations inside packet serialization and physics tick loops.


Linux io_uring & Thread-per-Core Runtime



Replace legacy epoll syscall overhead with Linux io_uring (tokio-uring or glommio) using shared user/kernel ring buffers to eliminate context-switching bottlenecks under heavy packet load.


Implement a Thread-per-Core execution model with explicit CPU core pinning (core_affinity). Assign isolated world instances/shards to dedicated physical cores and communicate across cores exclusively via lock-free SPSC/MPMC channels (crossbeam / flume), eliminating Mutex contention.


Kernel-Level Anti-DDoS & Packet Filtering (eBPF / XDP)



Provide C-based eBPF / XDP hooks designed to attach at the NIC driver layer before packets reach user-space Rust sockets.


Drop SYN floods, malformed Protocol 47 handshakes, oversized VarInt headers, and ping-bot floods at line rate (millions of packets/sec) with zero CPU impact on the game engine.


Native C/Zig FFI Compression (libdeflate)



Replace pure-Rust flate2/zlib bottlenecks with FFI bindings to libdeflate (via libdeflater or custom C/Zig bindings) for 3x faster Zlib/Deflate compression and decompression of chunk payloads and batched packet frames.


Offload heavy compression tasks to a dedicated worker pool (rayon) so the 50ms tick loop never stalls on CPU-bound compression.


Data-Oriented ECS & Cache-Aligned Memory



Architect all entities, players, projectiles, and ticking blocks using a cache-friendly Entity Component System (ECS) (bevy_ecs, hecs, or custom dense generational arenas) backed by contiguous flat Vec arrays rather than pointer-heavy OOP graphs, maximizing L1/L2 CPU cache hits.


Sandboxed Hot-Reloadable Game Logic (Luau / WebAssembly)



Embed a sandboxed Luau (mlua) or WebAssembly (wasmtime) runtime exposed through a strict Rust host API.


Enable 0ms hot-reloading of minigames (Capture the Wool, SkyWars, BedWars), custom kits, knockback profiles, and event listeners in production without recompiling or restarting the Rust binary, while guaranteeing memory isolation against untrusted scripts.


Maintain 100% wire compatibility with unmodified Vanilla, Lunar Client, Badlion, Feather, and Forge 1.8.8/1.8.9 clients while applying aggressive internal transport optimizations:


Tick-Window Packet Batching / Coalescing



Accumulate all outgoing packets destined for a player during the 50ms tick window into a single contiguous BytesMut frame, then compress and flush them in a single io_uring write operation. Achieve 25–40% bandwidth savings through superior cross-packet Deflate dictionary compression and reduced TCP/IP header overhead.


Dynamic Compression Threshold



Implement adaptive compression thresholds (256–512 bytes). Bypass Zlib framing entirely for tiny high-frequency packets (KeepAlive, relative entity moves, head yaw/pitch) to avoid payload inflation and CPU waste, while applying maximum libdeflate compression to chunk and bulk entity packets.


Server-Side Frustum / Occlusion Culling & Distance-Adaptive Tick Rates



Visibility Culling: Implement fast voxel ray-marching / occlusion checks so entity movement, equipment, and animation packets are suppressed for players fully occluded behind thick solid geometry or outside the receiver's frustum.


Adaptive Update Frequency: Broadcast entity state at full 20 Hz within close combat range (< 16 blocks) for frame-accurate PvP, while throttling distant entities (> 48–64 blocks) to 5–10 Hz with smooth interpolation, cutting lobby/large-battle bandwidth by up to 50%.


Zero-Copy Shared Packet Broadcasting (bytes::Bytes)



Serialize and compress shared broadcast packets (MapChunkBulk, multi-block changes, world particles, global chat) once into reference-counted immutable bytes::Bytes, fanning out cheap pointer clones across all recipient player queues with zero memory duplication.


Protocol 47 Completeness & Strict Server Authority



Implement all remaining secondary Protocol 47 packets: Maps (0x34), UpdateSign (0x33), SpawnParticle (0x2A), Effect (0x28), Statistics (0x37), context-aware TabComplete (0x3A), SetSlot (0x2F) for external windows, PlayerAbilities (0x39), ChangeGameState (0x2B), and Respawn (0x07).


Enforce strict server-authoritative sanitization on every inbound packet: reject NaN/Infinity coordinates, oversized strings, malicious NBT nesting/book crashers, invalid slot indices, and impossible movement deltas.


Ephemeral Flat-Binary World Format + Zstd (Minigames / Arenas)



Implement a custom read-only flat-binary world format (Slime/Polar-inspired) compressed with Zstandard (zstd) using paletted block containers.


Load entire arena maps directly from disk/cache into RAM in < 2ms, execute matches purely in memory with zero disk writes, and drop the Arc<World> on match end without leaving filesystem clutter.


Anvil World Persistence (.mca Regions for Persistent Modes)



Implement concurrent, non-blocking .mca region file reading and writing using memory-mapped I/O (memmap2) paired with libdeflate worker pools.


Support the full NBT chunk storage specification: 16x16x16 sections, block IDs/metadata, block/sky light arrays, biomes, heightmaps, entities, and tile entities.


Native WorldManager (Multiverse-Equivalent) & Cancelable Chunk Queue



Build a multi-world orchestrator (WorldManager) managing concurrent independent worlds (lobby, ctw_arenas, world_nether, world_the_end) on isolated async tasks.


Support instant cross-world player transfers (flushing old chunk columns and sending Respawn 0x07 + new chunk packets without disconnecting the client).


Prioritized Generation Queue: Order chunk loading/generation by player look vector and velocity; immediately cancel and drop queued chunk tasks when a player teleports away or disconnects.


Competitive 1.8 PvP, Knockback & Hitbox Physics



Implement exact vanilla 1.8.8/1.8.9 combat mechanics: configurable knockback vectors (horizontal, vertical, sprint-reset, air friction), 0.5s damage immunity cooldown (hurt_resistant_time), critical hit detection, block-hitting mitigation, and lag-compensated AABB raycast hitboxes.


Implement all environmental damage pipelines: fall distance calculation, fire/lava ticks, drowning, potion/poison ticks, void damage, and saturation/hunger-based natural regeneration.


Entities, AI & Spatial Pathfinding (coral-server/src/mob/[ai.rs](http://ai.rs/))



Implement cache-friendly entity state machines, sensory perception loops, and grid-optimized A pathfinding* backed by $O(1)$ spatial hashing grids.


Implement natural mob spawning governed by light levels, biome rules, and mob caps, alongside aggressive distance-based despawning.


Component-Driven Redstone & Block Physics



Build a high-speed Redstone propagation engine supporting signal strength decay (0–15), repeaters (1–4 tick delays), comparators (compare/subtract modes), sticky/normal pistons with block-pushing limits, dispensers, droppers, hoppers, TNT priming, and doors/trapdoors/pressure plates.


Enforce per-tick update budgets and cycle detection to eliminate infinite-loop stack overflows and lag machines.


Crafting, Furnaces, Anvils & Enchantments



Implement 2x2 and 3x3 shaped/shapeless crafting validators, atomic shift-click and drag inventory transactions, furnace smelting with fuel burn timers, anvil repair/renaming/combining mechanics, and bookshelf-scaled enchantment table seed calculations.


Villagers, Trading, Nether & End Dimensions



Implement villager profession state machines, deterministic trade unlocking, and Iron Golem village spawning.


Build Nether and End terrain/structure generators (Nether fortresses, Obsidian pillars, End crystals), portal transition triggers, Enderman teleportation AI, and the Ender Dragon boss fight state machine.


Vanilla JSON Chat Engine & Anti-Spam



Implement full JSON text component serialization/deserialization (clickEvent, hoverEvent, insertion, color/formatting codes), \n multiline support, server-to-console bridging, token-bucket chat spam rate limiters, and dynamic argument tab-completion.


Target Selectors (@a, @p, @r, @e) & Granular Permissions



Build a zero-allocation selector parser supporting spatial, score, and state filters ([x=, y=, z=, r=, rm=, m=, c=, l=, lm=, type=, name=, team=, score_X=, score_X_min=]). Every command must resolve target selectors seamlessly.


Replace binary is_op checks with a granular 0–4 permission level hierarchy integrated with external permission nodes.


Complete Command Registry (~40 Vanilla + World Commands)



Implement all missing gameplay and administrative commands: give, clear, effect, enchant, xp/experience, spawnpoint, setblock, fill, clone, testfor, testforblock, blockdata, playsound, particle, gamerule, weather, defaultgamemode, seed, save-all/save-on/save-off, stop, ban/ban-ip/pardon/pardon-ip/banlist, list, scoreboard, team, title, tellraw, help, me, execute, spreadplayers, worldborder, debug, plus native world management (/world load|unload|import|clone|tp).


Stateless Edge Proxy & Limbo/Queue Layer



Design compatibility with a high-throughput Rust/Go reverse TCP proxy layer (Anycast/GeoDNS) supporting cryptographic IP/UUID forwarding signatures (preventing direct-connect UUID spoofing) and seamless backend server switching without client disconnects.


Support ultra-lightweight Limbo / Queue routing for overflow traffic spikes (< 0.2 MB RAM per queued player).


Distributed State Mesh & High-Throughput Persistence



Integrate async pub/sub and in-memory state connectors (NATS and DragonflyDB / Redis) for sub-millisecond cross-node synchronization (global chat, parties, friends, matchmaking queues, server heartbeats).


Implement non-blocking persistence drivers (sqlx / ScyllaDB connector) with connection pooling and write-behind caching for millions of player profiles, stats, and inventories.


Cloud-Native Orchestration & Observability



Expose a gRPC/HTTP control plane compatible with Agones / Nomad / Docker orchestrators to spin up or tear down ephemeral arena nodes in < 50ms based on real-time queue demand.


Export lock-free atomic telemetry metrics (Prometheus/OpenTelemetry): 99th-percentile tick duration, packet batch compression ratio, chunk cache hit rate, and active ECS entity counts.