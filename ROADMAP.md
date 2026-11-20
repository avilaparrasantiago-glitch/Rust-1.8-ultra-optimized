# ROADMAP — Coral

Documento vivo. Registra el estado comprobado por área (con evidencia archivo:línea) y las tareas atómicas pendientes, ordenadas por dependencias. Se actualiza al cerrar cada bloque con lo que cambió, la evidencia y el siguiente bloque.

Reglas de bloque (AGENTS.md): una tarea atómica por iteración, TDD, sin `.unwrap()`/`.expect()` en rutas de red/tick, cero alloc en heap en el loop de 20 TPS, `cargo check` + `clippy -D warnings` + `nextest` obligatorios antes de cerrar, commit por bloque.

---

## Estado comprobado (2026-11-20)

Verificado leyendo los 22 módulos de `src/` (5.947 líneas). Clasificación: CUBIERTA / PARCIAL / FALTA.

### 1. Runtime y transporte — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| io_uring en Linux | CUBIERTO | main.rs:79-82, network.rs:28-29, `Socket::read/write` network.rs:33-69 |
| Batching 50 ms ≤192 paquetes + pool rayon deflate | CUBIERTO | network.rs:82-113, 390-423 |
| Umbral dinámico de compresión 256/512 | CUBIERTO | network.rs:295-301, wire.rs:164-196 |
| Zero-copy broadcast `Packet(Arc<Shared>)` + `OnceLock<Bytes>` | CUBIERTO | wire.rs:199-233 |
| Loop de tick 50 ms | CUBIERTO | engine.rs:375-492 |
| Thread-per-core con pinning de I/O | FALTA | main.rs:83-91 usa `current_thread`+`LocalSet`; pinning solo en hilos de sim por mundo (engine.rs:299-303) |

### 2. Protocolo 47 — CUBIERTA

| Aspecto | Estado | Evidencia |
|---|---|---|
| Salida secundaria: respawn 0x07, abilities 0x39, game_state 0x2b, sign 0x33, effect 0x28, particle 0x2a, stats 0x37, tab-complete 0x3a, set_slot 0x2f, map 0x34 | CUBIERTO | packets.rs:55-197 |
| Entrada 0x00–0x19 | CUBIERTA | input.rs:107-289 |
| Sanitización: VarInt ≤5 bytes, floats no finitos, coords ≤30M, límites de string, MAX_FRAME/MAX_BODY, profundidad/presupuesto NBT en slots | CUBIERTO | wire.rs:9-10, 15-31, 78-109; input.rs:45-105 |

### 3. Mundo — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Formato plano CORAL47 + zstd (template/load/save) | CUBIERTO | arena.rs:115/141/176 (cap 64 MiB) |
| `Column::packet` | FALTA meshing | arena.rs:89-100 manda u16 por bloque sin comprimir |
| Anvil .mca import/export | CUBIERTO | anvil.rs:156/333; memmap2 solo snapshot anónimo (189-191) |
| Manager multi-mundo con carriles, transfer Respawn, cola priorizada/cancelable (2 cols/tick, unload) | CUBIERTO | engine.rs:203-348, 415-431, 693-738 |
| Overlay de ediciones `Arc<Edits>` | CUBIERTO | arena.rs:283-307 |

### 4. Gameplay — PARCIAL

| Aspecto | Estado | Evidencia |
| Combate 1.8: attack, LOS, críticos, knockback k=0.4/0.8 sprint; daño caída/vacío, regen, hardness | CUBIERTO | engine.rs:809-854, 925-1019, 1106; grid visibilidad 1020-1091; invuln 10 ticks engine.rs:164-180 |
| Inventario de jugador de 45 casillas | CUBIERTO | inventory.rs y engine/windows.rs gestionan clic, shift, barra rápida, arrastre y recogida por doble clic |
| Ventanas de mesa y horno | PARCIAL | engine/windows.rs recoge de las casillas visibles. Siguen pendientes yunque, encantamiento, dispensador y los intercambios numéricos en casillas propias de la mesa o el horno. |
| Crafting 9 recetas + furnace | CUBIERTO | crafting.rs:164-220 — sin enchant/anvil/brewing |
| Redstone: dust, repeaters, doors, gates, pistons ≤12, levers; budget 512 updates/tick | PARCIAL | redstone.rs:36-130 — sin comparadores |
| Comandos ~30 + tab completion, op por UUID | CUBIERTO (parcial vs ~40) | commands.rs:61-69, 526-559 |
| Selectores @a/@e/@p/@r con filtros | CUBIERTO | selector.rs:32-158 |
| Entidades no jugador | PARCIAL | `src/entities.rs` almacena objetos, orbes, flechas y mobs por mundo. Quedan pendientes su aparición en el juego y la difusión a clientes. |
| Permisos 0–4 granulares | FALTA | commands.rs:61-69 es op por UUID |

### 5. Scripting — CUBIERTA (mínima)

| Aspecto | Estado | Evidencia |
|---|---|---|
| Sandbox Luau (TABLE\|STRING\|MATH, 4 MiB, interrupt 30ms/3ms) | CUBIERTO | scripts.rs:66-90 |
| Hot reload 500 ms | CUBIERTO | scripts.rs:158-168 |
| Host API: 1 handler de eventos, sin RW de mundo desde Lua | PARCIAL | scripts.rs:83-127 |

### 6. Distribuido — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Store autosave + flock | CUBIERTO | persistence.rs:18-76 |
| sqlx AnyPool lease/claim + spool de disco | CUBIERTO | profiles.rs:129-231 |
| Métricas counters + histograma 11 buckets, /metrics /health | CUBIERTO | metrics.rs:32-74 |
| Seed persistente, rechazo de listeners públicos sin forwarding | CUBIERTO | config.rs:88-116, 59-61 |
| Forwarding HMAC Coral47/1 + replay nonce | CUBIERTO | identity.rs:40-99 |
| NATS chat bus + Redis heartbeat | CUBIERTO | mesh.rs:25, 49-51 |

Extras verificados: coral_xdp.c (token bucket SYN v4+v6, LRU per-CPU 16384, ~40 SYN/s @ dport 25565), tools/rust_mcp.py, setup-linux.sh, tests/protocol47.py (3 pruebas: transfer 82, promoción de cola 120, forwarding firmado 133), benches/transport.rs (criterion).

Higiene: repositorio Git local inicializado. La instantánea original de 36 archivos está en `main` (commit `02d8688`).

---

## Tareas atómicas por dependencias

Orden: núcleo de memoria/ECS y Protocolo 47 antes que comandos y orquestación. Cada tarea = un bloque AGENTS.md (TDD, check+clippy+nextest, commit).

### Fase 0 — Higiene (prerrequisito de todo commit)

- [x] **T0.1** Repositorio local y reglas de exclusión. La rama `feat/higiene` parte de `02d8688`. `.gitignore` conserva las reglas de `/target/` y `/data/` y añade `*.log` y `__pycache__/`.
  - `tests/repository.rs` enumera los archivos que Git incluiría en un repositorio temporal. Con las reglas originales aparecían dos logs y una caché de Python. Con las reglas nuevas solo aparecen los archivos de proyecto previstos.
  - Cargo incorporó a `Cargo.lock` los 24 paquetes que faltaban para resolver Criterion 0.5.1, ya declarado en el manifiesto. En Windows, `cargo check --workspace` y `cargo clippy --workspace -- -D warnings` terminaron sin advertencias. `cargo nextest run --workspace --locked` pasó la prueba de exclusiones (1/1).

### Fase 1 — Núcleo de memoria/ECS

- [x] **T1.1** Cada `Room` reserva 4096 plazas antes de arrancar su bucle. `EntityArena` en `src/entities.rs` mantiene las entidades activas contiguas, reutiliza las plazas liberadas y rechaza identificadores caducados. `Room::run` avanza la edad y la posición una vez por tick.
  - `tests/entities.rs` verifica los cuatro tipos, el límite de capacidad, la retirada con compactación y la reutilización de identificadores. En Windows x64, con 4096 entidades y tras un tick de calentamiento, 200 ciclos de retirada, inserción y tick registraron cero asignaciones en el hilo de la prueba. Cargo check y Clippy limpios, nextest 4/4.
- [ ] **T1.2** Broadcast de spawn/move/destroy de entidades vía el zero-copy `Packet(Arc<Shared>)` (wire.rs:199-233) respetando el grid de visibilidad.
- [ ] **T1.3** Drops de items al romper bloque + pickup por AABB (reutilizar física de engine.rs:925-973).
- [ ] **T1.4** Orbes de XP (spawn al matar, imán, recoger) sobre la misma arena.

### Fase 2 — Protocolo 47: chunks y culling

- [ ] **T2.1** Contenedores paletted + meshing de columnas: reemplazar el u16-por-bloque de `Column::packet` (arena.rs:89-100) por paletted sections con varint-packed block storage, comprimidas con el pool libdeflate existente (network.rs:390-423).
- [ ] **T2.2** MapChunkBulk: agrupar columnas contiguas en un solo paquete 0x26 con continuación bitmask.
- [ ] **T2.3** Culling de entidades: frustum + occlusion por ray-marching de voxeles para suprimir packets de entidad fuera de vista; throttle adaptativo 20 Hz <16 bloques / 5–10 Hz >48 (usando el grid engine.rs:1020-1091).

### Fase 3 — Runtime thread-per-core

- [ ] **T3.1** Pinning del hilo de I/O con core_affinity + canales SPSC crossbeam entre I/O y sim (main.rs:83-91, engine.rs:299-303).
- [ ] **T3.2** Dedicar un hilo por conexión con buffers BytesMut reutilizados, eliminando el reparto del LocalSet.

### Fase 4 — Gameplay

- [ ] **T4.1** Ventanas anvil y enchant (windows.rs:207 líneas) + validación anvil/enchant en crafting.rs.
- [ ] **T4.2** Comparadores y hoppers en redstone.rs (modo compare/subtract, budget 512 ya existente).
- [ ] **T4.3** Mobs sobre T1.1: AI por máquinas de estado, A* en grid, spawning por luz/bioma, despawn por distancia.
- [x] **T4.4** Inventario del jugador. Verificado el intercambio existente con la barra rápida e implementada la recogida por doble clic en `src/inventory.rs`. En `src/engine/windows.rs` la recogida recorre las casillas de la ventana abierta y excluye el resultado virtual de la mesa.
  - `tests/inventory.rs` reproduce el rechazo anterior del modo 6 y comprueba prioridad de pilas parciales, capacidad, metadatos y botones inválidos. `tests/protocol47.py` contrasta confirmaciones, casillas y cursor con vectores independientes por TCP.
  - Windows, perfil dev. Check y Clippy sin advertencias, nextest 2/2 y TCP 4/4. Un contador de asignaciones alrededor de 10.000 pares de clics de modos 2 y 6, tras calentamiento, registró cero. En Windows la suite TCP usa una URL SQLite explícita porque la URL predeterminada con ruta absoluta falló al arrancar.
- [ ] **T4.5** Villagers + trading + Nether/End (portales, dragon fight) — último de gameplay, depende de T1.1 y T2.1.

### Fase 5 — Comandos y permisos

- [ ] **T5.1** Permisos 0–4 granulares reemplazando el check de UUID (commands.rs:61-69), integrado con profiles.rs.
- [ ] **T5.2** Comandos faltantes (~10): give, clear, effect, enchant, xp, spawnpoint, setblock, fill, clone, testfor(block), blockdata, playsound, particle, gamerule, weather, save-all, ban/pardon, scoreboard/team, tellraw, execute, spreadplayers, worldborder. Selector.rs ya resuelve @a/@e/@p/@r.
- [ ] **T5.3** Comandos de mundo /world load|unload|import|clone|tp sobre el Manager (engine.rs:203-348).
- [ ] **T5.4** Host API Luau con RW de mundo (setblock/getblock/tp) usando la misma autoridad que comandos.

### Fase 6 — Distribuido y orquestación

- [ ] **T6.1** Control plane gRPC para Agones/Nomad (spin up/down <50 ms), sobre metrics.rs /health.
- [ ] **T6.2** Write-behind ScyllaDB (sqlx) para perfiles masivos, manteniendo lease/claim de profiles.rs:129-231.
- [ ] **T6.3** Tooling de attach eBPF/XDP + tests de línea (coral_xdp.c ya filtra SYN; falta carga/attach/documentado en setup).
- [ ] **T6.4** Capa edge proxy/limbo: compat de forwarding con firmas criptográficas (identity.rs Coral47/1 ya existe) y routing a limbo <0.2 MB/jugador.

---

## Siguiente bloque

**T1.2** — Integrar las entidades de cada arena con la visibilidad por jugador y difundir su aparición, movimiento y retirada mediante paquetes compartidos de Protocolo 47.
