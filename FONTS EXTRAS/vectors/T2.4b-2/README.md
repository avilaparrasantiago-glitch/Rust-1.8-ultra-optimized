# Vectores de estructuras del Overworld

Referencia: servidor dedicado Minecraft Java 1.8.9, SHA-1
`b58b2ceb36e01bcd8dbf49c8fb66c55a9f0676cd`. No se comparan hashes de contenedores con orden arbitrario de claves: se comparan los campos NBT, los arrays y las listas ordenadas.

* `*.nbt.gz`: comienzos y piezas de la semilla 12345; `structure-loot.nbt.gz` contiene además los slots y el siguiente entero del RNG para 128 semillas por familia.
* `stronghold/terrain.gz` y `mineshaft/terrain.gz`: fase individual de estructura sobre terreno tallado real. Cada ventana de población usa la semilla oficial de población; se recorren X y luego Z. Se registra el siguiente entero del RNG y después el volumen de estados en orden Y,Z,X. La fortaleza añade al final las entidades de bloque en NBT.
* `stronghold/whole.gz`, `whole-mine.gz`, `portal-room.gz`, `mine-room.gz`, `mine-corridor.gz`: casos aislados sobre piedra. El caso de pasillos usa la semilla 84, con seis vagonetas.
* `stronghold/shapes.gz`: operaciones locales de colocación por clase, orientación, puerta y variante. Rust aplica el recorte, consultas de terreno, RNG, bloques, metadatos y botín durante la generación; no se utilizan columnas precalculadas.

Para regenerar las formas con JDK 8, desde la raíz en PowerShell:

```powershell
$ref = 'FONTS EXTRAS'
$out = "$ref/target/tmp"
$cp = "$out;$ref/target;$ref/target/tmp/server-1.8.9.jar"
javac -cp $cp -d $out "$ref/tools/TerrainReference.java" "$ref/tools/StrongholdShapes.java"
java -cp $cp StrongholdShapes source "$out/ShapeProbes.java"
javac -cp $cp -d $out "$out/ShapeProbes.java"
java -cp $cp StrongholdShapes capture "$ref/vectors/T2.4b-2/stronghold/shapes.gz"
```

Para contrastar de nuevo un stronghold completo, ejecutar `TerrainReference 12345 -44 59 stronghold-layout <salida-inicio.gz> <salida-terreno.gz> terrain` con ese classpath. Para la mina, usar `12345 -31 -32 mineshaft-layout`.
