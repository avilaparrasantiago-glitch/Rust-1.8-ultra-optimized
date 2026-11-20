# SyRust · instrucciones para Codex

SyRust es un proyecto propio. Usar exclusivamente las fuentes de Minecraft que el usuario aportó en los dos textos de referencia. El repositorio externo Coral queda excluido como fuente de código, diseño y documentación. Implementar en Rust y verificar el comportamiento con los servidores oficiales indicados.

Lee ARCHITECTURE.md y ROADMAP.md al iniciar cada chat. El primero contiene el encargo original. El segundo recoge el estado comprobado. Elige un único punto del roadmap por iteración.

1. Identifica la entrada real del subsistema, los archivos que vas a cambiar y la prueba que demostrará su funcionamiento. Para transporte, empieza por batching y después por difusión compartida.
2. Consulta Cargo.lock, las firmas de rust-analyzer y el código de la versión resuelta. En Context7 especifica la versión del crate. Corrobora las respuestas con Cargo. Consulta bevy_ecs y wasmtime solo si el bloque requiere esas dependencias.
3. Crea una rama feat/<subsistema> desde una base comprobada. Reproduce el fallo con una prueba, implementa el cambio y vuelve a ejecutar la prueba. Para Protocolo 47 compara los bytes con vectores independientes.
4. Nunca uses .unwrap() ni .expect() en rutas críticas de red o ticks. Propaga errores con Result o thiserror y maneja las desconexiones.
5. Mantén cero asignaciones en el heap dentro del bucle principal de 20 TPS. Reserva y reutiliza Vec, BytesMut y colas antes de entrar. Mide las asignaciones después del calentamiento.
6. Antes de cerrar una tarea ejecuta obligatoriamente `cargo check --workspace` y `cargo clippy --workspace -- -D warnings`. Exige cero errores y cero advertencias. Ejecuta `cargo nextest run --workspace --locked` y las pruebas TCP si cambia la red.
7. Para rendimiento usa Criterion y cargo flamegraph con símbolos y carga representativa. Registra CPU, sistema, perfil, parámetros y distribución de tiempos. Mide carga fría, carga desde caché e instanciación por separado.
8. Con los controles en verde revisa el diff y crea un commit local limitado al bloque. Actualiza ROADMAP.md con archivos, evidencia, pendientes y el siguiente bloque. Para comenzar un chat limpio, usa esos dos documentos como relevo. Publicar cambios o abrir un chat de usuario requiere su instrucción.
