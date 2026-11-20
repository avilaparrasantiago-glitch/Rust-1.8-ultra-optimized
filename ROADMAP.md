# ROADMAP — Coral

Documento vivo. Registra el estado comprobado por área (con evidencia archivo:línea) y las tareas atómicas pendientes, ordenadas por dependencias. Se actualiza al cerrar cada bloque con lo que cambió, la evidencia y el siguiente bloque.

Reglas de bloque (AGENTS.md): una tarea atómica por iteración, TDD, sin `.unwrap()`/`.expect()` en rutas de red/tick, cero alloc en heap en el loop de 20 TPS, `cargo check` + `clippy -D warnings` + `nextest` obligatorios antes de cerrar, commit por bloque.

---

## Estado comprobado (2026-11-20)

Revisión de la base `396734a` y del bloque T5.1. «Probado» identifica un recorrido ejecutado en Windows x64. «En código» identifica una ruta inspeccionada. Los objetivos de escala de ARCHITECTURE.md siguen pendientes de medición con carga representativa.

### 1. Runtime y transporte — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| Batching y compresión | `network.rs`, `Workers::batch` y escritor de sesión. Hasta 192 paquetes por ventana de 50 ms, codificación por paquete con libdeflate en Rayon y concatenación para el socket. El umbral 256/512 se negocia al entrar. TCP probado. | Medir coste y ahorro bajo carga. Protocolo 47 conserva un frame independiente por paquete. |
| Difusión y tick | `wire.rs`, `Packet::encoded`; `engine/entity_view_tests.rs` prueba buffers compartidos y cero asignaciones en el recorrido de entidades. | El tick completo sigue creando colecciones y paquetes, por ejemplo en `Room::visibility` y `Room::advance`. Faltan mediciones de TPS, p99, RAM y densidad por núcleo. |
| I/O y afinidad | `main.rs` inicia tokio-uring en Linux. `network.rs` usa sus sockets. `Manager::create` intenta fijar el hilo de cada mundo. | Runtime de I/O por núcleo, comprobación del pinning y prueba de la ruta io_uring en Linux. |

### 2. Protocolo 47 — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| Codificación y estados | `packets.rs` implementa los diez constructores secundarios solicitados. Las pruebas TCP ejercitan login, status, juego, transferencia, inventario, estadísticas y completado. Los vectores en `entity_view_tests.rs` cubren las entidades de T1.2. | El constructor de mapas carece de productor de juego. Faltan vectores y pruebas funcionales para el resto del catálogo y sesiones con clientes reales. |
| Entrada y autoridad | `input::decode` reconoce 0x00–0x19. `wire::Cursor`, el lector NBT y `Room::movement` limitan tamaños, profundidad, valores no finitos y movimiento. | Varias ramas devuelven `Action::Ignore`, entre ellas interacción con vehículos, confirmación de transacción y recursos. Falta demostrar comportamiento y validación por acción. |

### 3. Mundo — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| CORAL47 y chunks | `arena.rs` guarda paletas e índices empaquetados con Zstd. `Column::packet` escribe estados u16 little-endian, iluminación y biomas para 1.8. La suite TCP comprobó longitud y máscara de la columna inicial. | Medir carga fría, caché e instanciación por separado frente al objetivo de 2 ms. Falta MapChunkBulk. |
| Anvil | `anvil.rs`, `Store::open` y CLI. En la auditoría, crear arena plana → exportar .mca → importar → servir por TCP produjo el mismo paquete 0x21 para (0,0). | Cobertura de NBT de mundos reales, entidades persistentes y tile entities. La lectura copia a un mapa anónimo y la importación inicial es síncrona. |
| Mundos y visibilidad | `Manager`, `Room::advance` y `World::set`. Transferencia con Respawn probada por TCP. Cola por distancia, orientación y desplazamiento; dos columnas por tick. Superposición de ediciones sobre la plantilla. | Medir cancelación bajo carga, implementar frustum y verificar las rutas administrativas de importación y clonación. |

### 4. Gameplay — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| Combate y supervivencia | `Room::movement`, `Room::attack`, `Player::harm` y `Room::advance` contienen alcance, visión, críticos, knockback, caída, vacío y regeneración básica. | Raycast AABB con compensación de latencia, perfiles de knockback, bloqueo, daño ambiental, hambre y pociones. |
| Inventario, crafting y redstone | T4.4 tiene prueba Rust y vectores TCP. `crafting.rs`, `engine/windows.rs` y `redstone.rs` contienen recetas, horno y propagación con presupuesto. | Paridad de recetas, transacciones, yunque, encantamientos, comparadores, hoppers y restantes mecanismos. |
| Entidades | T1.1 y T1.2 tienen pruebas de capacidad, generaciones y difusión. La rotura en supervivencia crea objetos que caen y se recogen por contacto (T1.3). | Orbes, IA, búsqueda de caminos, comercio y dimensiones. |
| Comandos y autorización | `commands::REGISTRY`, `allowed`, `Selector::resolve`. Niveles 0–4 y nodos por UUID. T5.1 comprueba la integración con SQL. | El selector @e solo recorre jugadores. El registro, el chat y los subcomandos de mundo cubren parte del encargo. |

### 5. Scripting — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| Luau y recarga | `Game::load`, `Game::event` y `Scripts::start` configuran sandbox, 4 MiB, límites temporales y revisión del archivo cada 500 ms. Dos clientes recibieron respuestas distintas al evento de entrada antes y después de editar el script, en el mismo proceso. | Medir latencias de recarga y comportamiento con carga. Probar límites de recursos y recuperación de errores. |
| API del host | `Room::notify_script` y `Room::script_replies` conectan entrada, colocación, muerte y baja con mensajes, kit y modo de juego. | Lectura y edición autorizada del mundo, teletransporte, perfiles de knockback y modos de juego completos. |

### 6. Distribuido — PARCIAL

| Aspecto | Evidencia | Pendiente |
|---|---|---|
| Perfiles y persistencia | `profiles.rs` contiene SQLx, leases, checkpoints y spool. La admisión y la lectura de permisos se probaron contra SQLite. `persistence.rs` usa un escritor de regiones y bloqueo de archivo. | Verificar PostgreSQL, recuperación tras caída, rendimiento y backend ScyllaDB. |
| Malla, proxy y cola | `mesh.rs` publica chat en NATS y heartbeat en Redis. La suite TCP ejercita forwarding firmado, rechazo de repetición y promoción de cola. | Ensayo con dos nodos y servicios reales, grupos, amigos, matchmaking, conmutación de backend y consumo de memoria en limbo. |
| Métricas, orquestación y XDP | `metrics.rs` expone HTTP, histograma de duración y contadores de bytes y caché. `linux/coral_xdp.c` contiene el filtro SYN. | Métrica de entidades activas, control plane gRPC y validación de XDP en una NIC Linux. |

**Falta implementación** de I/O por núcleo (T3.1–T3.2), MapChunkBulk (T2.2), orbes (T1.4), IA y comercio (T4.3/T4.5), API Luau de mundo (T5.4), control plane y ScyllaDB (T6.1–T6.2). Los ensayos Linux, multinodo y de carga son verificaciones adicionales. La evidencia de esta revisión fue `cargo check --workspace`, Clippy con advertencias como errores, nextest 8/8, TCP 5/5 y las comprobaciones locales de Anvil y recarga Luau descritas arriba.

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
- [x] **T1.2** Difusión de aparición, movimiento y retirada desde `Room::visibility`. El índice por columna respeta las columnas cargadas, la distancia y la línea de visión. El observador distingue generaciones al reutilizar una plaza. Los identificadores de red de estas entidades empiezan en 2³⁰.
  - `src/engine/entity_view.rs`, `src/wire.rs` y `src/network.rs` reutilizan paquetes compartidos y una cola de salida acotada. Si una versión sigue en tránsito, el siguiente recorrido reintenta el envío con la posición vigente.
  - `src/engine/entity_view_tests.rs` comprueba vectores de Protocolo 47, dos observadores, reutilización de plaza y entrega por tandas de 400 apariciones y retiradas. En Windows x64, perfil de pruebas, el recorrido de visibilidad de 32 mobs durante 200 ticks tras calentamiento encoló 6400 movimientos con cero asignaciones en el hilo medido. Check y Clippy limpios, nextest 8/8 y regresiones TCP 4/4.
- [x] **T1.3** La rotura válida en supervivencia reserva la entidad antes de editar el bloque. En creativo se edita sin generar objeto. `src/engine/drops.rs` aplica caída, espera de 10 ticks, recogida por caja de colisión y caducidad a los 6000 ticks. Reutiliza el índice de jugadores por columna y los buffers de casilla; conserva el objeto si falla la inserción en inventario.
  - `tests/protocol47.py` verifica dos observadores del mismo objeto, metadatos del bloque de tierra, recogida por el segundo jugador, retirada para ambos y casilla vacía del minero. La prueba Rust cubre inventario lleno, numeración de ventana de horno, reintento y cero asignaciones durante 200 recogidas después del calentamiento.
  - Se conserva la equivalencia bloque/objeto previa (césped a tierra). Tablas completas de botín, herramientas y encantamientos siguen pendientes.
- [ ] **T1.4** Orbes de XP (spawn al matar, imán, recoger) sobre la misma arena.

### Fase 2 — Protocolo 47: chunks y culling

- [x] **T2.1** Paleta e índices empaquetados en disco y RAM ya presentes. La columna en red conserva el formato 1.8. La medición de rendimiento de arena sigue pendiente.
- [ ] **T2.2** MapChunkBulk: agrupar columnas contiguas en un solo paquete 0x26 con continuación bitmask.
- [ ] **T2.3** Culling de entidades: frustum + occlusion por ray-marching de voxeles para suprimir packets de entidad fuera de vista; throttle adaptativo 20 Hz <16 bloques / 5–10 Hz >48 (usando el grid engine.rs:1020-1091).

### Fase 3 — Runtime thread-per-core

- [ ] **T3.1** Pinning del hilo de I/O con core_affinity + canales SPSC crossbeam entre I/O y sim (main.rs:83-91, engine.rs:299-303).
- [ ] **T3.2** Repartir las conexiones entre los bucles de I/O por núcleo y reutilizar los buffers por conexión.

### Fase 4 — Gameplay

- [ ] **T4.1** Ventanas anvil y enchant (windows.rs:207 líneas) + validación anvil/enchant en crafting.rs.
- [ ] **T4.2** Comparadores y hoppers en redstone.rs (modo compare/subtract, budget 512 ya existente).
- [ ] **T4.3** Mobs sobre T1.1: AI por máquinas de estado, A* en grid, spawning por luz/bioma, despawn por distancia.
- [x] **T4.4** Inventario del jugador. Verificado el intercambio existente con la barra rápida e implementada la recogida por doble clic en `src/inventory.rs`. En `src/engine/windows.rs` la recogida recorre las casillas de la ventana abierta y excluye el resultado virtual de la mesa.
  - `tests/inventory.rs` reproduce el rechazo anterior del modo 6 y comprueba prioridad de pilas parciales, capacidad, metadatos y botones inválidos. `tests/protocol47.py` contrasta confirmaciones, casillas y cursor con vectores independientes por TCP.
  - Windows, perfil dev. Check y Clippy sin advertencias, nextest 2/2 y TCP 4/4. Un contador de asignaciones alrededor de 10.000 pares de clics de modos 2 y 6, tras calentamiento, registró cero. En Windows la suite TCP usa una URL SQLite explícita porque la URL predeterminada con ruta absoluta falló al arrancar.
- [ ] **T4.5** Villagers + trading + Nether/End (portales, dragon fight) — último de gameplay, depende de T1.1 y T2.1.

### Fase 5 — Comandos y permisos

- [x] **T5.1** Al reclamar el perfil se leen nivel y nodos de `coral_permissions(uuid,level,nodes)` en la misma transacción. Los nodos son una lista JSON. Los permisos SQL y de configuración son aditivos y se aplican al ejecutar, pedir ayuda o completar comandos. Las modificaciones SQL entran en vigor en el siguiente login; el autosave solo escribe datos de juego. Se verificaron nivel 1, un nodo de nivel 2 y el rechazo de otro comando de ese mismo nivel por TCP.
- [ ] **T5.2** Completar los comandos ausentes del registro, incluidos effect, enchant, blockdata, gamerule, defaultgamemode, stop, ban/pardon, execute, spreadplayers, worldborder y debug; ampliar scoreboard y team.
- [ ] **T5.3** Comandos de mundo /world load|unload|import|clone|tp sobre el Manager (engine.rs:203-348).
- [ ] **T5.4** Host API Luau con RW de mundo (setblock/getblock/tp) usando la misma autoridad que comandos.

### Fase 6 — Distribuido y orquestación

- [ ] **T6.1** Control plane gRPC para Agones/Nomad (spin up/down <50 ms), sobre metrics.rs /health.
- [ ] **T6.2** Write-behind ScyllaDB con un driver compatible para perfiles masivos, manteniendo lease/claim de profiles.rs:129-231.
- [ ] **T6.3** Tooling de attach eBPF/XDP + tests de línea (coral_xdp.c ya filtra SYN; falta carga/attach/documentado en setup).
- [ ] **T6.4** Capa edge proxy/limbo: compat de forwarding con firmas criptográficas (identity.rs Coral47/1 ya existe) y routing a limbo <0.2 MB/jugador.

---

## Siguiente bloque

**T1.4** — Crear orbes de experiencia al matar, acercarlos a jugadores elegibles y acreditar XP una sola vez al recogerlos. Verificar aparición, actualización de experiencia y retirada por TCP.
