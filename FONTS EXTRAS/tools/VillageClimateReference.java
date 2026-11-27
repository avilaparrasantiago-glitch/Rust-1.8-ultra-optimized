import java.io.*;
import java.nio.file.*;
import java.util.Random;

/** Captures unpopulated village starts and climate samples from the supplied 1.8.9 server. */
public class VillageClimateReference {
    static final class Flat extends TerrainReference.World {
        java.util.Map<Long,alz> blocks = new java.util.HashMap<>();
        java.util.Map<Long,aky> tiles = new java.util.HashMap<>();
        java.util.List<String> spawns = new java.util.ArrayList<>();
        Flat(long seed){super(seed);t.a(this);}
        public alz p(cj p){return blocks.getOrDefault(p.g(),p.o()<64?afi.b.Q():afi.a.Q());}
        public boolean d(cj p){return p(p).c()==afi.a;}
        public boolean a(cj p,alz s,int flags){blocks.put(p.g(),s);return true;}
        public cj r(cj p){int y=255;while(y>=0 && !p(new cj(p.n(),y,p.p())).c().t().c())y--;return new cj(p.n(),y+1,p.p());}
        public akw s(cj p){if(p(p).c()!=afi.ae)return null;aky chest=tiles.computeIfAbsent(p.g(),k->new aky());chest.a(p);return chest;}
        public ok E(cj p){return new ok(oj.b,0,0,0);}
        public boolean d(pk mob){
            spawns.add(pm.b(mob)+" "+(int)Math.floor(mob.s)+" "+(int)mob.t+" "+(int)Math.floor(mob.u)+" "+Float.floatToIntBits(mob.y));
            return true;
        }
        public void b(cj p,afh block){}
    }
    static void population(Path output,long seed,aqu start)throws Exception{
        Flat w=new Flat(seed);
        aqe b=start.a();
        Random r=new Random(1234);
        for(int cx=(b.a-8)>>4;cx<=(b.d-8)>>4;cx++)for(int cz=(b.c-8)>>4;cz<=(b.f-8)>>4;cz++)
            start.a(w,r,new aqe(cx*16+8,cz*16+8,cx*16+23,cz*16+23));
        try(DataOutputStream out=new DataOutputStream(new java.util.zip.GZIPOutputStream(
                Files.newOutputStream(output.resolve("placed-"+seed+".bin.gz"))))){
            out.writeLong(r.nextLong());
            for(int x=b.a-2;x<=b.d+2;x++)for(int z=b.c-2;z<=b.f+2;z++)for(int y=48;y<=80;y++){
                alz s=w.p(new cj(x,y,z));out.writeShort(afh.a(s.c())*16+s.c().c(s));
            }
        }
    }
    public static void main(String[] args) throws Exception {
        kb.c();
        Path output = Files.createDirectories(Paths.get(args[0]));
        Flat sample=new Flat(12345);
        // The runtime gives world and population randomness independent streams.
        for(long spawnSeed=0; ;spawnSeed++){
            Random stream=new Random(spawnSeed);
            sample.s.setSeed(12345L^0x5deece66dL);sample.s.nextLong();
            adt.a(sample,ady.e(1),8,8,16,16,stream);
            if(!sample.spawns.isEmpty()){
                try(PrintWriter out=new PrintWriter(output.resolve("plains-group.txt").toFile(),"UTF-8")){
                    out.println(spawnSeed+" "+stream.nextLong());
                    for(String mob:sample.spawns)out.println(mob);
                }
                break;
            }
        }
        cj below=new cj(0,64,0);
        try(DataOutputStream out=new DataOutputStream(Files.newOutputStream(output.resolve("block-climate.bin")))){
            for(int id=0;id<256;id++){
                afh block=afh.c(id);
                if(block==null)block=afi.a;
                sample.a(below,block.Q(),2);
                out.writeByte(Math.min(15,block.p()));
                out.writeByte(block.r());
                out.writeByte((afh.c(78).d(sample,below.a())?1:0)|(block.v()?2:0)
                    |(block.t().c()||block.t().d()?4:0));
            }
        }
        java.lang.reflect.Field lootField = aqw.j.class.getDeclaredField("a");
        lootField.setAccessible(true);
        Random lootRandom = new Random(12345);
        aky chest = new aky();
        chest.a(new cj(-17,65,23));
        ob.a(lootRandom,(java.util.List)lootField.get(null),chest,3+lootRandom.nextInt(6));
        dn chestTag = new dn();
        chest.b(chestTag);
        try (OutputStream stream=Files.newOutputStream(output.resolve("blacksmith-chest.nbt.gz"))) {
            dx.a(chestTag,stream);
        }
        try (PrintWriter out = new PrintWriter(output.resolve("passive-spawns.tsv").toFile(), "UTF-8")) {
            java.lang.reflect.Field namesField = pm.class.getDeclaredField("d");
            namesField.setAccessible(true);
            java.util.Map names = (java.util.Map)namesField.get(null);
            for (ady biome : ady.n()) {
                if (biome == null) continue;
                for (ady.c entry : biome.a(pt.b))
                    out.printf("%d\t%s\t%d\t%d\t%d%n",biome.az,names.get(entry.b),entry.a,entry.c,entry.d);
            }
        }
        try (PrintWriter out = new PrintWriter(output.resolve("temperatures.tsv").toFile(), "UTF-8")) {
            for (ady biome : ady.n()) {
                if (biome == null) continue;
                for (int[] p : new int[][] {{0,64,0}, {-39,96,18}, {16,255,-17}}) {
                    int bits = Float.floatToIntBits(biome.a(new cj(p[0],p[1],p[2])));
                    out.printf("%d\t%d\t%d\t%d\t%08x%n", biome.az,p[0],p[1],p[2],bits);
                }
            }
        }
        for (long seed : new long[]{12345,-9581}) {
            aqv generator = new aqv();
            generator.c = new TerrainReference.World(seed) {
                public ady b(cj pos) { return v().a(pos); }
            };
            boolean captured = false;
            outer: for (int x=-128; x<128; x++) for (int z=-128; z<128; z++) {
                if (!generator.a(x,z)) continue;
                Random base = new Random(seed);
                generator.b.setSeed(seed ^ x*base.nextLong() ^ z*base.nextLong());
                generator.b.nextInt();
                aqu start = generator.b(x,z);
                if (!start.d()) continue;
                String name = String.format("village-%d-%d-%d.nbt.gz",seed,x,z);
                try (OutputStream stream = Files.newOutputStream(output.resolve(name))) {
                    dx.a(start.a(x,z),stream);
                }
                population(output,seed,start);
                captured = true;
                break outer;
            }
            if (!captured) throw new AssertionError("missing village for seed " + seed);
        }
    }
}
