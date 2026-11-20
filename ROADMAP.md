# ROADMAP — Coral

Documento vivo. Registra el estado comprobado por área (con evidencia archivo:línea) y las tareas atómicas pendientes, ordenadas por dependencias. Se actualiza al cerrar cada bloque con lo que cambió, la evidencia y el siguiente bloque.

Reglas de bloque (AGENTS.md): una tarea atómica por iteración, TDD, sin `.unwrap()`/`.expect()` en rutas de red/tick, cero alloc en heap en el loop de 20 TPS, `cargo check` + `clippy -D warnings` + `nextest` obligatorios antes de cerrar, commit por bloque.

---

## Estado comprobado (2026-11-20)

Estado contrastado con los puntos de entrada del código. Las referencias describen qué está implementado. Los objetivos de rendimiento y la compatibilidad con clientes requieren pruebas y mediciones propias.

### 1. Runtime y transporte — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| io_uring en Linux | CUBIERTO | main.rs:79-82, network.rs:28-29, `Socket::read/write` network.rs:33-69 |
| Batching de 50 ms, hasta 192 paquetes y compresión con rayon/libdeflate | CUBIERTO | network.rs:82-113, 390-423 agrupa frames del Protocolo 47 para una escritura y comprime cada frame por separado. Ahorro de ancho de banda sin medir. |
| Umbral de compresión 256/512 | CUBIERTO | network.rs:295-301, wire.rs:164-196 fija el umbral en el login según la carga. |
| Zero-copy broadcast `Packet(Arc<Shared>)` + `OnceLock<Bytes>` | CUBIERTO | wire.rs:199-233 |
| Loop de tick 50 ms | CUBIERTO | engine.rs:375-492 |
| Cero asignaciones en el tick | PARCIAL | engine.rs, `Room::visibility`, reconstruye colecciones por jugador. También se crean paquetes durante el tick. Falta medir el loop completo bajo carga. |
| Frecuencia adaptativa y oclusión | PARCIAL | engine.rs, `Room::visibility`, usa distancia y línea de visión para jugadores. Falta frustum y extenderlo a entidades no jugador. |
| Thread-per-core con pinning de I/O | FALTA | main.rs:83-91 usa `current_thread`+`LocalSet`; pinning solo en hilos de sim por mundo (engine.rs:299-303) |

### 2. Protocolo 47 — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Salida secundaria: respawn 0x07, abilities 0x39, game_state 0x2b, sign 0x33, effect 0x28, particle 0x2a, stats 0x37, tab-complete 0x3a, set_slot 0x2f, map 0x34 | CUBIERTO | packets.rs:55-197 |
| Entrada 0x00–0x19 | CUBIERTA | input.rs:107-289 |
| Sanitización: VarInt ≤5 bytes, floats no finitos, coords ≤30M, límites de string, MAX_FRAME/MAX_BODY, profundidad/presupuesto NBT en slots | CUBIERTO | wire.rs:9-10, 15-31, 78-109; input.rs:45-105 |
| Compatibilidad con todos los clientes y semántica Vanilla | PENDIENTE DE VERIFICACIÓN | tests/protocol47.py cubre transferencia, promoción de cola y forwarding mediante un cliente de prueba. |

### 3. Mundo — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Formato plano CORAL47 + zstd con paletas | CUBIERTO | arena.rs, `Section::read`, `Template::save` y `Template::load` (límite descomprimido de 64 MiB). Objetivo de carga <2 ms sin medir. |
| Serialización de columnas | PARCIAL | arena.rs, `Column::packet`, expande las paletas a u16 little-endian del Protocolo 47. wire.rs, `Packet::encoded`, aplica compresión a nivel de frame. |
| Anvil .mca import/export | PARCIAL | anvil.rs, `import` y `export`, implementan lectura y escritura. memmap2 usa un snapshot anónimo y persistence.rs delega la exportación al worker; la importación se ejecuta al abrir el mundo. |
| Manager multi-mundo con carriles, transfer Respawn, cola priorizada/cancelable (2 cols/tick, unload) | CUBIERTO | engine.rs:203-348, 415-431, 693-738 |
| Overlay de ediciones `Arc<Edits>` | CUBIERTO | arena.rs:283-307 |

### 4. Gameplay — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Combate y daño ambiental 1.8 | PARCIAL | engine.rs, `attack`, `harm` y `advance` cubren LOS, críticos, knockback, invulnerabilidad, caída, vacío y regeneración. Falta contrastar la paridad de PvP y completar las fuentes de daño ambiental. |
| Inventario de 45 slots | PARCIAL | inventory.rs, `Inventory::click`, admite modos 0/1/2 y 5, incluidas teclas numéricas. Falta doble clic para recoger. |
| Ventanas de mesa y horno | PARCIAL | engine/windows.rs tiene mesa y horno. Quedan anvil, enchant y dispenser, además de teclas numéricas y doble clic en contenedores. |
| Crafting 9 recetas + furnace | PARCIAL | crafting.rs:164-220. Quedan enchant, anvil y brewing. |
| Redstone | PARCIAL | redstone.rs, `Circuit::step` y `output`, cubren polvo, repetidores, puertas, trampillas, pistones y fuentes de señal básicas con un presupuesto de 512 actualizaciones por tick. Quedan comparadores, tolvas, dispensadores, soltadores y encendido de TNT. |
| Comandos y autocompletado | PARCIAL | commands.rs, `REGISTRY`, `execute` y `complete`. Ver T5.2 y T5.3. |
| Selectores @a/@e/@p/@r con filtros | PARCIAL | selector.rs, `Selector::resolve`, filtra jugadores, reúne resultados en un Vec y los ordena. Faltan las entidades no jugador y la resolución sin asignaciones. |
| Entidades no jugador (ítems, orbes de XP, flechas, mobs) | PARCIAL | entities.rs aporta almacenamiento y actualización de posición y edad. engine.rs integra la arena en cada mundo. La difusión ya usa las columnas cargadas y un radio de 80 bloques. Los bloques sueltan objetos que se recogen por proximidad. Las bajas PvP producen orbes de experiencia. Los mobs quedan para T4.3. |
| Permisos 0–4 granulares | PARCIAL | commands.rs, `allowed`, consulta niveles por UUID y nodos `coral.command.*` declarados en config.rs. Falta integración con profiles.rs. |

### 5. Scripting — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Sandbox Luau (TABLE\|STRING\|MATH, 4 MiB, interrupt 30ms/3ms) | CUBIERTO | scripts.rs:66-90 |
| Hot reload 500 ms | CUBIERTO | scripts.rs:158-168 |
| Host API: 1 handler de eventos, sin RW de mundo desde Lua | PARCIAL | scripts.rs:83-127 |

### 6. Distribuido — PARCIAL

| Aspecto | Estado | Evidencia |
|---|---|---|
| Guardado periódico y bloqueo exclusivo de mundo | CUBIERTO | persistence.rs, `Store::open`, `save` y `close` usan `File::try_lock` y un hilo de exportación. |
| sqlx AnyPool lease/claim + spool de disco | CUBIERTO | profiles.rs, `Profiles::open` y `claim`, y `writer`. La ruta SQLite local se codifica antes de construir la URL. |
| Métricas y /health | PARCIAL | metrics.rs expone histograma del tick, bytes y aciertos de caché. Falta el contador de entidades activas. |
| Seed persistente, rechazo de listeners públicos sin forwarding | CUBIERTO | config.rs:88-116, 59-61 |
| Forwarding HMAC Coral47/1 + replay nonce | CUBIERTO | identity.rs:40-99 |
| NATS chat bus + Redis heartbeat | CUBIERTO | mesh.rs:25, 49-51 |

Extras verificados en el código: linux/coral_xdp.c limita SYN IPv4/IPv6 en el puerto 25565. Quedan el despliegue y las pruebas de línea (T6.3). tests/protocol47.py contiene cinco pruebas TCP. benches/transport.rs define el benchmark de Criterion.

Higiene: repositorio Git activo. `git check-ignore` confirmó las exclusiones de target/, data/ y *.log. Cargo.lock incluye Criterion y sus dependencias para que las comprobaciones con `--locked` puedan resolver los benchmarks declarados en Cargo.toml.

---

## Tareas atómicas por dependencias

El desarrollo continúa en este repositorio. Los subsistemas nuevos tendrán diseño e implementación propios, conservando la compatibilidad con clientes Minecraft Java 1.8.8 y 1.8.9.

Orden: núcleo de memoria/ECS y Protocolo 47 antes que comandos y orquestación. Cada tarea = un bloque AGENTS.md (TDD, check+clippy+nextest, commit).

### Fase 0 — Higiene (prerrequisito de todo commit)

- [x] **T0.1** Repositorio Git activo desde la importación inicial. .gitignore excluye target/, data/ y *.log.
- [x] **T0.2** Corregir el arranque con SQLite local. `Profiles::open` conserva la ruta completa en una URL SQLite codificada. Nextest verificó una ruta con espacios, `#` y `%`. Las tres pruebas TCP pasaron en Windows.

### Fase 1 — Núcleo de memoria/ECS

- [x] **T1.1** Arena generacional de entidades no jugador. `src/entities.rs` usa un Vec denso, reutiliza slots sin reutilizar identificadores obsoletos y comparte el cálculo de columnas con la visibilidad de jugadores. `Room::run` actualiza una arena de 1.024 plazas reservadas antes del tick. `tests/entities.rs` verificó la reutilización, el movimiento entre columnas y cero asignaciones en 1.024 ciclos de inserción, tick y borrado tras el calentamiento.
- [x] **T1.2** Broadcast de spawn/move/destroy de entidades vía el zero-copy `Packet(Arc<Shared>)` (wire.rs:199-233) respetando el grid de visibilidad.
- [x] **T1.3** Drops de items al romper bloque + pickup por AABB (reutilizar física de engine.rs:925-973).
- [x] **T1.4** Orbes de XP (spawn al matar, imán, recoger) sobre la misma arena.

### Fase 2 — Protocolo 47: chunks y culling

- [x] **T2.1** Optimizar la expansión de las paletas internas de `Section` al serializar columnas. Conservar el formato u16 little-endian de Protocolo 47 y verificar los bytes con un vector independiente. Medir la primera serialización y la lectura desde caché.
- [ ] **T2.2** MapChunkBulk: agrupar columnas contiguas en un solo paquete 0x26 con continuación bitmask.
- [ ] **T2.3** Culling de entidades: frustum + occlusion por ray-marching de voxeles para suprimir packets de entidad fuera de vista; throttle adaptativo 20 Hz <16 bloques / 5–10 Hz >48 (usando el grid engine.rs:1020-1091).

- [ ] **T2.4a** Generador propio de Overworld normal al estilo de Java 1.8. Obtener relieve, costas, océanos, ríos y biomas a partir de la semilla guardada. Comprobar continuidad de chunks, incluidas coordenadas negativas, y reproducibilidad con distinto orden de carga.
- [ ] **T2.4b** Poblar el terreno con cuevas, minerales, vegetación y estructuras según el bioma. Verificar las formaciones que cruzan chunks y un punto de aparición transitable.
- [ ] **T2.4c** Integrar la generación bajo demanda con carga, caché y persistencia de ediciones. Medir tiempo por chunk en frío y desde caché, memoria y p99 de tick durante la exploración con varios jugadores.

**Criterios de aceptación de T2.4.** Usar la semilla persistente y comprobar con clientes Java 1.8.8/1.8.9 que el terreno normal se puede explorar, editar y volver a cargar. Generar columnas fuera del hilo de simulación, priorizar las que necesita el jugador y cancelar solicitudes obsoletas al teletransportarse o salir. Acotar las colas y la caché para que explorar terreno nuevo tenga un coste de memoria controlado.

Contrastar los resultados con [ARCHITECTURE.md](ARCHITECTURE.md). Durante exploración simultánea y teletransportes, registrar TPS, p99 y número de ticks que superan el presupuesto de 50 ms, memoria por jugador activo (objetivo aproximado de 1,5–3 MB) y jugadores por núcleo físico. Verificar cero E/S de disco bloqueante en el hilo de simulación y medir asignaciones tras el calentamiento en serialización y física. Medir por separado generación en frío, acceso desde caché e instanciación de arenas efímeras desde RAM. Para estas arenas aplican el objetivo de instanciación inferior a 2 ms y el límite de 150 MB por mundo de minijuego cargado. Publicar hardware, perfil de compilación, semilla, distancia de visión y carga utilizada junto con los resultados.

### Fase 3 — Runtime thread-per-core

- [ ] **T3.1** Pinning del hilo de I/O con core_affinity + canales SPSC crossbeam entre I/O y sim (main.rs:83-91, engine.rs:299-303).
- [ ] **T3.2** Distribuir las conexiones entre los hilos de I/O fijados por núcleo, con buffers BytesMut reutilizados, en lugar de concentrarlas en un único LocalSet.

### Fase 4 — Gameplay

- [ ] **T4.1** Ventanas anvil y enchant (windows.rs:207 líneas) + validación anvil/enchant en crafting.rs.
- [ ] **T4.2** Comparadores y hoppers en redstone.rs (modo compare/subtract, budget 512 ya existente).
- [ ] **T4.3** Mobs sobre T1.1: AI por máquinas de estado, A* en grid, spawning por luz/bioma, despawn por distancia.
- [ ] **T4.4** Completar teclas numéricas en contenedores y doble clic para recoger en inventario y contenedores.
- [ ] **T4.5** Villagers + trading + Nether/End (portales, dragon fight) — último de gameplay, depende de T1.1 y T2.1.

### Fase 5 — Comandos y permisos

- [ ] **T5.1** Integrar con profiles.rs los niveles 0–4 y los nodos de permiso que commands.rs ya consulta en config.rs.
- [ ] **T5.2** Completar comandos pendientes del registro de ARCHITECTURE.md, incluidos effect, enchant, blockdata, gamerule, defaultgamemode, stop, ban/pardon, execute, spreadplayers, worldborder y debug. Revisar la cobertura parcial de scoreboard/team.
- [ ] **T5.3** Completar `/world import` y distinguir la semántica de load y clone, que hoy llaman a `Manager::create`. list, unload y tp ya tienen rutas propias.
- [ ] **T5.4** Host API Luau con RW de mundo (setblock/getblock/tp) usando la misma autoridad que comandos.

### Fase 6 — Distribuido y orquestación

- [ ] **T6.1** Control plane gRPC para Agones/Nomad (spin up/down <50 ms), sobre metrics.rs /health.
- [ ] **T6.2** Write-behind ScyllaDB (sqlx) para perfiles masivos, manteniendo lease/claim de profiles.rs:129-231.
- [ ] **T6.3** Tooling de attach eBPF/XDP + tests de línea (coral_xdp.c ya filtra SYN; falta carga/attach/documentado en setup).
- [ ] **T6.4** Capa edge proxy/limbo: compat de forwarding con firmas criptográficas (identity.rs Coral47/1 ya existe) y routing a limbo <0.2 MB/jugador.

---

## Siguiente bloque

**T2.2** — Agrupar columnas en paquetes MapChunkBulk (0x26) y verificar la recepción mediante Protocolo 47.

La prueba de engine/entity_visibility.rs cubre paquetes compartidos, columnas descargadas y reutilización de slots. Los vectores de aparición siguen el esquema 1.8 de PrismarineJS. La generación de entidades durante el juego corresponde a los bloques siguientes.

T1.3 se verificó con rotura de hierba, aparición de tierra, recogida por AABB e inventario sincronizado sobre TCP. src/engine/drops.rs prueba la recogida por otro jugador cuando el primer inventario está lleno. La espera inicial de recogida es de diez ticks.

T1.4 se verificó con dos clientes TCP: baja PvP, aparición y movimiento del orbe, recogida, actualización de XP y retirada de la entidad. El orbe transfiere siete puntos por nivel de la víctima hasta un máximo de cien. La víctima conserva el resto.

Última verificación de T1.4 en Windows GNU con Rust 1.100.0 y Nextest 0.9.150. Pasaron `cargo check --workspace --locked`, `cargo clippy --workspace -- -D warnings`, `cargo nextest run --workspace --locked` (6 pruebas) y `tests/protocol47.py` (5 pruebas TCP).

T2.1 — src/arena.rs expande secuencialmente los índices de 0 a 12 bits por bloque y reserva el paquete completo antes de escribirlo. El test TCP compara bloques, luz y biomas de una columna plana con un vector independiente. En 1.024 lecturas de una columna ya serializada, el contador de asignaciones registró cero. Pasaron check, Clippy, 8 pruebas de Nextest y 5 pruebas TCP.

Medición T2.1 con Criterion 0.5.1 sobre Windows GNU, Intel Core i3-1005G1, Rust 1.100.0, perfil dev con símbolos y opt-level 3 para el paquete principal. Columna plana de una sección, ambos binarios ejecutados consecutivamente, un hilo de análisis, 1 s de calentamiento, 3 s de medición, 30 muestras. La primera serialización pasó de 31,87 µs (IC 95 % 31,14–32,84) a 13,33 µs (12,85–13,79). En caché pasó de 24,69 ns (24,15–25,48) a 24,33 ns (24,21–24,44). En mediciones separadas de 20 muestras, leer y descomprimir una plantilla de 81 columnas tardó 530,35 µs con el archivo en caché del sistema, y construir World con la plantilla en RAM tardó 153,06 ns. El perfil de CPU de 4 s se capturó con flamegraph a 99 Hz; el backend de Windows mostró direcciones sin resolver. Estas cifras corresponden al microbenchmark de serialización y no miden los TPS del servidor bajo carga.