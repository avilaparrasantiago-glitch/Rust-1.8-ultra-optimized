import java.io.*;
import io.netty.buffer.Unpooled;
import java.util.Random;
import java.lang.reflect.Field;

// Run against the dedicated 1.8 server, with its original obfuscated class names.
public final class TerrainReference {
    static class World extends adm {
        aec biomes;
        long seed;
        static ato settings(long seed) {
            dn tag = new dn();
            tag.a("RandomSeed", seed);
            return new ato(tag);
        }
        World(long seed) {
            super(null, settings(seed), new ano(), new nt(), false);
            this.seed = seed;
            biomes = new aec(seed, adr.b, "");
        }
        public aec v() { return biomes; }
        public long J() { return seed; }
        public ady b(cj pos) { return biomes.a(pos); }
        protected amv k() { return null; }
        protected int q() { return 0; }
    }
    static class SpikeWorld extends World {
        final int floor, hole;
        final java.util.List<String> trace = new java.util.ArrayList<>();
        SpikeWorld(long seed, int floor, int hole) {
            super(seed);
            this.floor = floor;
            this.hole = hole;
        }
        public boolean d(cj pos) { return pos.o() > floor; }
        public alz p(cj pos) {
            return (pos.o() == floor && pos.n() != hole ? afi.bH : afi.a).Q();
        }
        public boolean a(cj pos, alz state, int flags) {
            trace.add(pos.n()+" "+pos.o()+" "+pos.p()+" "+afh.a(state.c()));
            return true;
        }
        public boolean d(pk entity) {
            if (!(entity instanceof uf)) throw new AssertionError();
            trace.add("crystal "+(int)(2*entity.s)+" "+(int)entity.t+" "+(int)(2*entity.u)+" "+Float.floatToIntBits(entity.y));
            return true;
        }
    }
    public static void main(String[] args) throws Exception {
        kb.c();
        if (args[0].equals("properties")) {
            for (ady b : ady.n()) if (b != null)
                System.out.printf("%d %s %s%n", b.az, b.an, b.ao);
            return;
        }
        long seed = Long.parseLong(args[0]);
        int x = Integer.parseInt(args[1]), z = Integer.parseInt(args[2]);
        World world = new World(seed);
        if (args[3].equals("spike")) {
            SpikeWorld spike = new SpikeWorld(seed, 62, Integer.parseInt(args[5]));
            Random random = new Random(seed);
            boolean placed = new apq(afi.bH).b(spike, random, new cj(x, 63, z));
            spike.trace.add((placed ? "placed " : "failed ") + random.nextInt());
            java.nio.file.Files.write(java.nio.file.Paths.get(args[4]), (String.join("\n", spike.trace)+"\n").getBytes("UTF-8"));
            return;
        }
        if (args[3].equals("structure-starts")) {
            aqq[] structures = {new aqv(), new aqm(), new aqk(), new aqi()};
            for (aqq generator : structures) generator.c = world;
            try (OutputStream out = new BufferedOutputStream(new FileOutputStream(args[4]))) {
                for (int cz = -z; cz < z; cz++)
                    for (int cx = -x; cx < x; cx++) {
                        int mask = 0;
                        for (int i = 0; i < structures.length; i++)
                            if (structures[i].a(cx, cz)) mask |= 1 << i;
                        out.write(mask);
                    }
            }
            return;
        }
        if (args[3].equals("strongholds")) {
            aqo generator = new aqo();
            generator.c=world;
            generator.a(0,0);
            Field locations=aqo.class.getDeclaredField("g");
            locations.setAccessible(true);
            for(adg p:(adg[])locations.get(generator)) System.out.println(p);
            return;
        }
        aoa provider = new aoa(world, seed, false, "");
        if (args[3].equals("packet") || args[3].equals("bulk")) {
            amy column=provider.d(x,z);
            em wire=new em(Unpooled.buffer());
            if (args[3].equals("packet")) new go(column,true,65535).b(wire);
            else new gp(java.util.Arrays.asList(column,provider.d(x,z+1))).b(wire);
            byte[] payload=new byte[wire.readableBytes()];
            wire.readBytes(payload);
            java.nio.file.Files.write(java.nio.file.Paths.get(args[4]),payload);
            return;
        }
        if (args[3].equals("spawn")) {
            Random search = new Random(seed);
            cj start = world.v().a(0,0,256,world.v().a(),search);
            x = start == null ? 0 : start.n();
            z = start == null ? 0 : start.p();
            for (int attempt = 0; attempt <= 1000; attempt++) {
                amy column = provider.d(x >> 4,z >> 4);
                int y = 63;
                while (y < 255 && column.a(new cj(x,y+1,z)) != afi.a) y++;
                if (column.a(new cj(x,y,z)) == afi.c || attempt == 1000) {
                    System.out.printf("%d %d %d%n",x,y+1,z);
                    return;
                }
                x += search.nextInt(64) - search.nextInt(64);
                z += search.nextInt(64) - search.nextInt(64);
            }
        }
        Field field = aoa.class.getDeclaredField("h");
        field.setAccessible(true);
        ((Random)field.get(provider)).setSeed(x * 341873128712L + z * 132897987541L);
        ans primer = new ans();
        if (args[3].equals("end")) new aob(world,seed).a(x,z,primer);
        else if (args[3].startsWith("nether")) {
            anw nether = new anw(world,false,seed);
            nether.a(x,z,primer);
            if (!args[3].equals("nether")) {
                Field random = anw.class.getDeclaredField("j");
                random.setAccessible(true);
                ((Random)random.get(nether)).setSeed(x * 341873128712L + z * 132897987541L);
                nether.b(x,z,primer);
            }
            if (args[3].equals("nether-caves")) new anz().a(nether,world,x,z,primer);
        }
        else provider.a(x,z,primer);
        if (args[3].equals("density")) {
            Field density = aoa.class.getDeclaredField("p");
            density.setAccessible(true);
            try (DataOutputStream out = new DataOutputStream(new FileOutputStream(args[4]))) {
                for (double d : (double[])density.get(provider)) out.writeDouble(d);
            }
            return;
        }
        if (args[3].equals("surface") || args[3].equals("caves") || args[3].equals("ravines"))
            provider.a(x,z,primer,world.v().b(null,x*16,z*16,16,16));
        if (args[3].equals("caves") || args[3].equals("ravines"))
            new anx().a(provider,world,x,z,primer);
        if (args[3].equals("ravines")) new anr().a(provider,world,x,z,primer);
        try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[4])))) {
            for (int y=0;y<256;y++) for (int j=0;j<16;j++) for (int i=0;i<16;i++) {
                alz state=primer.a(i,y,j);
                out.writeShort((afh.a(state.c()) << 4) | state.c().c(state));
            }
        }
    }
}
