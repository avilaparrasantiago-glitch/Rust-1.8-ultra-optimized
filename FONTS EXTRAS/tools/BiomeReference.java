import java.io.BufferedOutputStream;
import java.io.FileOutputStream;

/** Compile against the dedicated 1.8.9 server. Run in a separate JVM per dataset. */
public final class BiomeReference {
    public static void main(String[] args) throws Exception {
        if (args.length != 7) {
            throw new IllegalArgumentException("layer seed minChunkX minChunkZ width height output");
        }
        kb.c();
        long seed = Long.parseLong(args[1]);
        ase layer;
        int size;
        switch (args[0]) {
        case "island":
            layer = new asd(1L);
            size = 16;
            break;
        case "fuzzy":
            layer = new asa(2000L, new asd(1L));
            size = 16;
            break;
        case "quarter":
        case "voronoi":
            boolean quarter = args[0].equals("quarter");
            layer = ase.a(seed, adr.b, "")[quarter ? 0 : 1];
            size = quarter ? 4 : 16;
            break;
        default:
            throw new IllegalArgumentException("unknown layer");
        }
        layer.a(seed);
        int x0 = Integer.parseInt(args[2]), z0 = Integer.parseInt(args[3]);
        int width = Integer.parseInt(args[4]), height = Integer.parseInt(args[5]);
        try (BufferedOutputStream out = new BufferedOutputStream(new FileOutputStream(args[6]))) {
            for (int z = 0; z < height; z++) {
                for (int x = 0; x < width; x++) {
                    asc.a();
                    int[] ids = layer.a((x0 + x) * size, (z0 + z) * size, size, size);
                    for (int i = 0; i < size * size; i++) out.write(ids[i]);
                }
            }
        }
    }
}
