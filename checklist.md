# Checklist de SyRust

## Fase 0 · Repositorio y arranque

- [x] T0.1 · Versionar el proyecto y versionar las referencias de `FONTS EXTRAS/` y excluir los artefactos locales.
- [x] T0.2 · Arrancar con una base SQLite local, incluso en rutas con espacios y caracteres especiales.

## Fase 1 · Entidades

- [x] T1.1 · Almacenar entidades en una arena que reutiliza memoria y conserva identificadores válidos.
- [x] T1.2 · Compartir paquetes de aparición, movimiento y retirada entre los jugadores que ven la entidad.
- [x] T1.3 · Soltar objetos al romper bloques y recogerlos por contacto.
- [x] T1.4 · Generar orbes de experiencia tras una baja y acreditar la experiencia al recogerlos.

## Fase 2 · Protocolo 47 y mundos

- [x] T2.1 · Serializar columnas desde sus paletas y verificar los bytes y el coste con y sin caché.
- [x] T2.2 · Enviar columnas contiguas en un paquete MapChunkBulk.
- [x] T2.3 · Filtrar entidades por campo de visión y oclusión, y ajustar la frecuencia de envío según la distancia.
- [x] T2.4a · Reproducir biomas, relieve, superficie y punto de aparición del Overworld DEFAULT.
- [ ] T2.4b · Completar y contrastar el tallado, las piezas de estructuras y la población por bioma del Overworld (incluyendo optimización `< 2 ms/col` en `populated.rs`: eliminar `fsync`/JSON+Gzip síncrono, caché `>= 2.048` LRU, `rayon` en batch para `primer()` y `heightmap` `O(1)`).
- [x] T2.4c · Entregar terreno al explorar, conservar mundos y ediciones al reabrir y medir el coste de exploración.
- [x] T2.4d · Generar terreno, cuevas, fortalezas y decoración del Nether, y conservar cofres y generadores al reabrir.
- [x] T2.4e · Generar la isla, los pilares, los cristales, la plataforma de llegada y la aparición inicial del dragón en el End.
- [ ] T2.4f · Desacoplar el radio de simulación activa (8–12 chunks) del streaming de horizonte lejano del lado del servidor (hasta 64 chunks / 1.024 bloques de radio) mediante anillo de solo lectura cacheado en `Arc<Bytes>`.

## Fase 3 · Ejecución por núcleo y sala elástica progresiva

- [ ] T3.1 · Fijar los hilos de entrada y salida a núcleos y conectarlos a la simulación mediante canales SPSC.
- [ ] T3.2 · Repartir conexiones entre esos hilos y reutilizar sus búferes.
- [ ] T3.3 · Estructurar el tick de 50 ms en cuatro fases deterministas (`Ingress` → `Core` → `Game` → `Egress`) e implementar la escalera progresiva de 7 niveles con 10 % de histéresis (`1–150` → `500` → `1.000` → `3.000` → `5.000` → `8.000` → `10.000+` jugadores por sala).

## Fase 4 · Mecánicas de juego

- [ ] T4.1 · Implementar ventanas y validación de operaciones de yunque y encantamiento.
- [ ] T4.2 · Verificar cada componente de redstone, su potencia y el orden de actualización en circuitos Vanilla.
- [ ] T4.3 · Implementar comportamiento de mobs, búsqueda de caminos, aparición y desaparición.
- [ ] T4.4 · Completar teclas numéricas en contenedores y recogida por doble clic en inventarios y contenedores.
- [ ] T4.5 · Completar aldeanos, comercio, gólems, viajes por portales y ciclo del dragón hasta el portal de salida.
- [ ] T4.6 · Reproducir el orden de los ticks de mundo y conservar eventos pendientes al guardar y reabrir.
- [ ] T4.7 · Contrastar colisiones, movimiento, daño, fusión y recogida de objetos con trazas por tick.
- [ ] T4.8 · Reproducir clima, sueño, rayos, nieve, hielo y ticks aleatorios de vegetación.

## Fase 5 · Comandos, permisos y scripting

- [ ] T5.1 · Integrar los niveles de permiso 0–4 y sus nodos con los perfiles persistentes.
- [ ] T5.2 · Verificar los 58 comandos dedicados, alias, selectores, permisos, ayuda, mensajes y efectos frente al servidor oficial.
- [ ] T5.3 · Implementar `/world import` y diferenciar la carga de un mundo de su clonación.
- [ ] T5.4 · Exponer lectura y edición de bloques y teletransporte a Luau con los mismos permisos que los comandos.

## Fase 6 · Distribución, proxy BVH y operación

- [ ] T6.1 · Controlar el arranque y la parada con gRPC y Agones/Nomad en menos de 50 ms.
- [ ] T6.2 · Escribir perfiles de forma diferida en ScyllaDB conservando la exclusividad de cada perfil.
- [ ] T6.3 · Documentar y probar la carga y conexión del filtro eBPF/XDP en la interfaz de red.
- [ ] T6.4 · Completar el proxy horizontal con IPC zero-copy `rkyv`, forwarding firmado, enrutamiento a limbo (<0,2 MB/jugador) y difusión espacial BVH con recorte progresivo de entidades visibles por cliente (`sin límite → 150 → 120 → 100 → 85 → 70 → 60`).
- [ ] T6.5 · Implementar generador de carga masiva en Rust (5.000–10.000 bots *headless* Protocolo 47) y perfilado Tracy para validar los 7 escalones progresivos y salas aisladas.

## Criterios transversales del roadmap

- [x] Usar io_uring en Linux y ejecutar el bucle de simulación a intervalos de 50 ms.
- [x] Agrupar hasta 192 paquetes por ventana, comprimir por trama y adaptar el umbral de compresión.
- [x] Procesar la entrada 0x00–0x19, validar tamaños y coordenadas y emitir los paquetes secundarios enumerados en el roadmap.
- [x] Guardar plantillas CORAL47 con paletas y zstd, superponer ediciones y gestionar transferencias y colas de columnas por mundo.
- [x] Aislar scripts Luau con límites de memoria y ejecución y recargarlos en caliente.
- [x] Guardar mundos periódicamente con bloqueo exclusivo, persistir la semilla y reservar perfiles con escritura diferida a disco.
- [x] Exigir forwarding firmado en listeners públicos, rechazar repeticiones y conectar chat NATS y heartbeat Redis.
- [ ] Verificar cero asignaciones en el tick completo tras el calentamiento y medir p99, ticks mayores de 50 ms y recursos bajo carga.
- [ ] Cuantificar el ahorro de ancho de banda del agrupamiento y del filtrado de entidades bajo carga.
- [ ] Verificar la carga de un mundo de arena en menos de 2 ms y completar la integración de persistencia Anvil.
- [ ] Verificar la carga de un mundo de arena en menos de 2 ms, la generación/población `< 2 ms/col` sin `fsync` bloqueante y el streaming de horizonte lejano hasta 64 chunks (1.024 bloques).
- [ ] Completar la compatibilidad de Protocolo 47 y la paridad de juego con clientes 1.8.8 y 1.8.9, incluido el lanzamiento del ojo de Ender.
- [ ] Contrastar población y botín del Nether con muestras independientes del servidor oficial.
- [ ] Documentar versión, huella, semilla, coordenadas y orden de población de cada mundo de referencia, incluidas semillas negativas y estructuras raras.
- [ ] Comparar bloques, luz, biomas, altura y NBT por sección Anvil antes y después de poblar, variando el orden de carga, la caché y los trabajadores.
- [ ] Completar la paridad de combate PvP, daño ambiental, dispensadores y elaboración de pociones.
- [ ] Resolver selectores sobre jugadores y otras entidades sin asignaciones.
- [ ] Exponer el contador de entidades activas junto a las métricas del servidor.
- [ ] Verificar las transiciones progresivas de los 7 niveles (con 10 % de histéresis) desde 150 hasta 10.000 bots simultáneos a 20.00 TPS constantes usando particionado espacial `rayon` y difusión BVH en proxy.