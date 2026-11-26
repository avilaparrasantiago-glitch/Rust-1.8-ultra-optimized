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
    static class EndWorld extends World {
        final java.util.Map<Long, ans> columns = new java.util.HashMap<>();
        final aob provider;
        final java.util.List<String> entities = new java.util.ArrayList<>();
        EndWorld(long seed) { super(seed); provider = new aob(this, seed); s.setSeed(seed); }
        ans column(int x, int z) {
            long key = ((long)x << 32) | (z & 0xffffffffL);
            return columns.computeIfAbsent(key, ignored -> {
                ans primer = new ans();
                provider.a(x,z,primer);
                return primer;
            });
        }
        public ady b(cj pos) { return ady.y; }
        public alz p(cj pos) {
            int y = pos.o();
            return y < 0 || y >= 256 ? afi.a.Q() : column(pos.n() >> 4,pos.p() >> 4).a(pos.n() & 15,y,pos.p() & 15);
        }
        public boolean d(cj pos) { return p(pos).c() == afi.a; }
        public cj r(cj pos) {
            int y = 255;
            while (y >= 0 && d(new cj(pos.n(), y, pos.p()))) y--;
            return new cj(pos.n(), y+1, pos.p());
        }
        public boolean a(cj pos, alz state, int flags) {
            column(pos.n() >> 4,pos.p() >> 4).a(pos.n() & 15,pos.o(),pos.p() & 15,state);
            return true;
        }
        public boolean d(pk entity) {
            entities.add(entity.getClass().getName()+" "+entity.s+" "+entity.t+" "+entity.u+" "+Float.floatToIntBits(entity.y));
            return true;
        }
    }
    static class FortressWorld extends EndWorld {
        final anw nether;
        final Random surface;
        FortressWorld(long seed) throws Exception {
            super(seed);
            nether=new anw(this,false,seed);
            Field f=anw.class.getDeclaredField("j");f.setAccessible(true);surface=(Random)f.get(nether);
        }
        ans column(int x,int z) {
            long key=((long)x<<32)|(z&0xffffffffL);
            return columns.computeIfAbsent(key,ignored->{
                ans col=new ans();
                nether.a(x,z,col);
                surface.setSeed(x*341873128712L+z*132897987541L);
                nether.b(x,z,col);
                new anz().a(nether,this,x,z,col);
                return col;
            });
        }
        public ady b(cj pos) { return ady.x; }
        public akw s(cj pos) {return null;}
        public void a(afh block,cj pos,Random random) {}
    }
    static class DesertWorld extends EndWorld {
        final aoa overworld;
        final Random surface;
        DesertWorld(long seed)throws Exception{
            super(seed);
            overworld=new aoa(this,seed,false,"");
            Field f=aoa.class.getDeclaredField("h");f.setAccessible(true);surface=(Random)f.get(overworld);
        }
        public ady b(cj pos){return v().a(pos);}
        ans column(int x,int z){
            long key=((long)x<<32)|(z&0xffffffffL);
            return columns.computeIfAbsent(key,ignored->{
                ans c=new ans();surface.setSeed(x*341873128712L+z*132897987541L);
                overworld.a(x,z,c);overworld.a(x,z,c,v().b(null,x*16,z*16,16,16));
                new anx().a(overworld,this,x,z,c);new anr().a(overworld,this,x,z,c);
                return c;
            });
        }
        public akw s(cj pos) {return null;}
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
        if (args[3].equals("mine-corridor")) {
            try (DataOutputStream out = new DataOutputStream(new java.util.zip.GZIPOutputStream(new FileOutputStream(args[4])))) {
                for (int facing=0;facing<4;facing++) for(int variant=0;variant<3;variant++) {
                    int width=facing%2==0?3:20, length=facing%2==0?20:3;
                    aqe box=new aqe(-9,32,503,-10+width,34,502+length);
                    dn tag=new dn();
                    tag.a("id","MSCorridor");tag.a("BB",new int[]{box.a,box.b,box.c,box.d,box.e,box.f});
                    tag.a("GD",1);tag.a("O",facing);tag.a("Num",4);tag.a("hr",variant==1);tag.a("sc",variant==2);
                    aqt corridor=aqr.b(tag,world);
                    java.util.Map<cj,alz> changes=new java.util.HashMap<>();
                    du carts=new du();
                    World stone=new World(seed){
                        public alz p(cj pos){return changes.getOrDefault(pos,afi.b.Q());}
                        public boolean a(cj pos,alz state,int flags){changes.put(pos,state);return true;}
                        public akw s(cj pos){return null;}
                        public boolean d(pk entity){
                            vb cart=(vb)entity;
                            dn entry=new dn(); entry.a("Pos",new int[]{(int)Math.floor(cart.s),(int)Math.floor(cart.t),(int)Math.floor(cart.u)});
                            du items=new du();
                            for(int slot=0;slot<cart.o_();slot++){zx stack=cart.a(slot);if(stack!=null){dn item=new dn();item.a("Slot",(byte)slot);items.a(stack.b(item));}}
                            entry.a("Items",items);carts.a(entry);return true;
                        }
                    };
                    for(int cx=(box.a-8)>>4;cx<=((box.d-8)>>4);cx++) for(int cz=(box.c-8)>>4;cz<=((box.f-8)>>4);cz++) {
                        Random rng=new Random(seed+31*cx+cz);
                        corridor.a(stone,rng,new aqe(cx*16+8,0,cz*16+8,cx*16+23,255,cz*16+23));
                        out.writeInt(rng.nextInt());
                    }
                    for(int y=31;y<=34;y++)for(int zp=box.c;zp<=box.f;zp++)for(int xp=box.a;xp<=box.d;xp++){
                        alz state=stone.p(new cj(xp,y,zp));out.writeShort(afh.a(state.c())*16+state.c().c(state));
                    }
                    dn entities=new dn();entities.a("Carts",carts);
                    if (carts.c()>0) System.out.printf("seed %d facing %d variant %d carts %d%n",seed,facing,variant,carts.c());
                    ByteArrayOutputStream raw=new ByteArrayOutputStream();
                    dx.a(entities,(DataOutput)new DataOutputStream(raw));
                    out.writeInt(raw.size());raw.writeTo(out);
                }
            }
            return;
        }
        if (args[3].equals("portal-room")) {
            try (DataOutputStream out = new DataOutputStream(new java.util.zip.GZIPOutputStream(new FileOutputStream(args[4])))) {
                for (int dir=0;dir<4;dir++) {
                    aqe box = new aqe(-4,32,505,-4+(dir%2==0?10:15),39,505+(dir%2==0?15:10));
                    aqp.g portal = new aqp.g(7,new Random(0),box,cq.b(dir));
                    java.util.Map<cj, alz> changes = new java.util.HashMap<>();
                    World stone = new World(seed) {
                        public alz p(cj pos) { return changes.getOrDefault(pos, afi.b.Q()); }
                        public akw s(cj pos) { return null; }
                        public boolean a(cj pos, alz state, int flags) { changes.put(pos,state);return true; }
                    };
                    for(int cx=(box.a-8)>>4;cx<=((box.d-8)>>4);cx++) for(int cz=(box.c-8)>>4;cz<=((box.f-8)>>4);cz++){
                        Random rng = new Random(seed+cx*31+cz);
                        portal.a(stone,rng,new aqe(cx*16+8,0,cz*16+8,cx*16+23,255,cz*16+23));
                        out.writeInt(rng.nextInt());
                    }
                    for(int y=32;y<=39;y++) for(int zp=box.c;zp<=box.f;zp++) for(int xp=box.a;xp<=box.d;xp++){
                        alz state=stone.p(new cj(xp,y,zp));
                        out.writeShort(afh.a(state.c())*16+state.c().c(state));
                    }
                }
            }
            return;
        }
        if (args[3].equals("structure-loot")) {
            Field library = aqp.e.class.getDeclaredField("a");
            library.setAccessible(true);
            java.lang.reflect.Method minePool = aqg.class.getDeclaredMethod("b");
            minePool.setAccessible(true);
            dn root = new dn();
            for (boolean mine : new boolean[]{false, true}) {
                du cases = new du();
                for (int sample = 0; sample < x; sample++) {
                    Random rng = new Random(seed + sample);
                    @SuppressWarnings("unchecked")
                    java.util.List<ob> pool = (java.util.List<ob>)(mine ? minePool.invoke(null) : library.get(null));
                    ob book = mine ? zy.cd.b(rng) : zy.cd.a(rng, 1, 5, 2);
                    int rolls = (mine ? 3 : 1) + rng.nextInt(4);
                    if (mine) rng.nextBoolean();
                    og inventory = mine ? new vb(world, 0.5, 20.5, 0.5) : new aky();
                    ob.a(rng, ob.a(pool, book), inventory, rolls);
                    du items = new du();
                    for (int slot = 0; slot < inventory.o_(); slot++) {
                        zx stack = inventory.a(slot);
                        if (stack == null) continue;
                        dn item = new dn();
                        item.a("Slot", (byte)slot);
                        items.a(stack.b(item));
                    }
                    dn result = new dn();
                    result.a("Items", items);
                    result.a("Next", rng.nextInt());
                    cases.a(result);
                }
                root.a(mine ? "Mine" : "Library", cases);
            }
            try (FileOutputStream out = new FileOutputStream(args[4])) { dx.a(root, out); }
            return;
        }
        if (args[3].equals("stronghold-layout") || args[3].equals("mineshaft-layout")) {
            aqq generator = args[3].equals("stronghold-layout") ? new aqo() : new aqf();
            generator.c = world;
            Random random = new Random(seed);
            generator.b.setSeed(seed ^ x * random.nextLong() ^ z * random.nextLong());
            generator.b.nextInt();
            if (!generator.a(x, z)) throw new IllegalArgumentException("not a start");
            aqu start = generator.b(x, z);
            if (args[3].equals("stronghold-layout")) {
                aqu reopened = aqr.a(start.a(x, z), world);
                for (aqu version : new aqu[]{start, reopened}) {
                    cj target = version.b().getFirst().a();
                    System.out.printf("eye %d %d %d%n",target.n(),target.o(),target.p());
                }
            }
            try (FileOutputStream out = new FileOutputStream(args[4])) {
                dx.a(start.a(x, z), out);
            }
            if (args.length > 5) {
                if (args.length>6 && (args[6].equals("whole") || args[6].equals("terrain"))) {
                    DesertWorld terrain=args[6].equals("terrain")?new DesertWorld(seed):null;
                    java.util.Map<cj,alz> changes=new java.util.HashMap<>();
                    java.util.Map<cj,akw> tiles=new java.util.HashMap<>();
                    World stone=new World(seed) {
                        public alz p(cj pos){alz state=changes.get(pos);return state!=null?state:(terrain==null?afi.b.Q():terrain.p(pos));}
                        public boolean a(cj pos,alz state,int flags){
                            if(state.c()!=p(pos).c()){
                                tiles.remove(pos);
                                akw tile=state.c()==afi.ae?new aky():state.c()==afi.ac?new all():null;
                                if(tile!=null){tile.a(pos);tiles.put(pos,tile);}
                            }
                            changes.put(pos,state);return true;
                        }
                        public akw s(cj pos){return tiles.get(pos);}
                        public void b(cj pos,akw tile){}
                        public boolean d(pk entity){return true;}
                    };
                    aqe bb=start.a();
                    try(DataOutputStream out=new DataOutputStream(new java.util.zip.GZIPOutputStream(new FileOutputStream(args[5])))){
                        for(int cx=(bb.a-8)>>4;cx<=((bb.d-8)>>4);cx++)for(int cz=(bb.c-8)>>4;cz<=((bb.f-8)>>4);cz++){
                            Random rng=new Random(seed);
                            if(terrain!=null){long mx=rng.nextLong()/2*2+1,mz=rng.nextLong()/2*2+1;rng.setSeed((cx*mx+cz*mz)^seed);}
                            else rng.setSeed(seed+cx*31+cz);
                            start.a(stone,rng,new aqe(cx*16+8,0,cz*16+8,cx*16+23,255,cz*16+23));
                            out.writeInt(rng.nextInt());
                        }
                        for(int y=bb.b-1;y<=bb.e;y++)for(int zp=bb.c;zp<=bb.f;zp++)for(int xp=bb.a;xp<=bb.d;xp++) {
                            alz state=stone.p(new cj(xp,y,zp));out.writeShort(afh.a(state.c())*16+state.c().c(state));
                        }
                        if(args[3].equals("stronghold-layout")){
                            dn root=new dn();for(akw tile:tiles.values()){dn tag=new dn();tile.b(tag);cj pos=tile.v();root.a(pos.n()+","+pos.o()+","+pos.p(),tag);}
                            dx.a(root,(DataOutput)out);
                        }
                    }
                    return;
                }
                aqt room = start.b().getFirst();
                aqe bb = room.c();
                java.util.Map<cj, alz> changes = new java.util.HashMap<>();
                World stone = new World(seed) {
                    public alz p(cj pos) { return changes.getOrDefault(pos, afi.b.Q()); }
                    public boolean a(cj pos, alz state, int flags) {
                        changes.put(new cj(pos.n(),pos.o(),pos.p()), state);
                        return true;
                    }
                };
                int cx = (bb.a-8)>>4, cz = (bb.c-8)>>4;
                for (int i=cx;i<=((bb.d-8)>>4);i++) for(int k=cz;k<=((bb.f-8)>>4);k++) {
                    room.a(stone,new Random(0),new aqe(i*16+8,0,k*16+8,i*16+23,255,k*16+23));
                }
                try(DataOutputStream out = new DataOutputStream(new java.util.zip.GZIPOutputStream(new FileOutputStream(args[5])))) {
                    for(int y=bb.b-1;y<=bb.e+1;y++) for(int zpos=bb.c-1;zpos<=bb.f+1;zpos++) for(int xpos=bb.a-1;xpos<=bb.d+1;xpos++) {
                        alz state=stone.p(new cj(xpos,y,zpos));
                        out.writeShort(afh.a(state.c())*16+state.c().c(state));
                    }
                }
            }
            return;
        }
        if(args[3].equals("desert-blocks")){
            DesertWorld desert=new DesertWorld(seed);
            aqm generator=new aqm();generator.c=desert;
            Random stream=new Random(seed);long mx=stream.nextLong(),mz=stream.nextLong();
            int count=0;
            try(DataOutputStream out=new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[4])))){
                for(int cz=-z;cz<z;cz++)for(int cx=-x;cx<x;cx++){
                    if(!generator.a(cx,cz)) continue;
                    generator.b.setSeed(seed ^ (cx*mx) ^ (cz*mz)); generator.b.nextInt();
                    aqu start=generator.b(cx,cz);
                    if(!(start.b().getFirst() instanceof aqn.a))continue;
                    System.out.println(cx+" "+cz);
                    start.a(desert,new Random(seed),new aqe(cx*16,0,cz*16,cx*16+31,255,cz*16+31));
                    for(int j=cz;j<=cz+1;j++)for(int i=cx;i<=cx+1;i++){
                        ans col=desert.column(i,j);
                        for(int y=0;y<256;y++)for(int dz=0;dz<16;dz++)for(int dx=0;dx<16;dx++){
                            alz b=col.a(dx,y,dz);out.writeShort((afh.a(b.c())<<4)|b.c().c(b));
                        }
                    }
                    if(++count==4)return;
                }
            }
            return;
        }
        if (args[3].equals("fortress-blocks")) {
            FortressWorld nether=new FortressWorld(seed);
            aqi gen=new aqi();gen.c=nether;
            if(!gen.a(x,z)) throw new IllegalArgumentException("not a fortress");
            aqu start=gen.b(x,z);
            int cx=Integer.parseInt(args[5]),cz=Integer.parseInt(args[6]);
            int side=Integer.parseInt(args[7]);
            aqe box=new aqe(cx*16,0,cz*16,(cx+side)*16-1,255,(cz+side)*16-1);
            start.a(nether,new Random(seed),box);
            try (DataOutputStream out=new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[4])))) {
                for(int j=cz;j<cz+side;j++)for(int i=cx;i<cx+side;i++){
                    ans c=nether.column(i,j);
                    for(int y=0;y<128;y++)for(int dz=0;dz<16;dz++)for(int dx=0;dx<16;dx++){
                        alz b=c.a(dx,y,dz);
                        out.writeShort((afh.a(b.c())<<4)|b.c().c(b));
                    }
                }
            }
            return;
        }
        if (args[3].equals("fortress-layout")) {
            aqi generator = new aqi();
            generator.c = world;
            try (PrintWriter out = new PrintWriter(args[4],"UTF-8")) {
                for (int cz=-z;cz<z;cz++) for(int cx=-x;cx<x;cx++) {
                    if (!generator.a(cx,cz)) continue;
                    aqu start = generator.b(cx,cz);
                    for (aqt piece : start.b()) {
                        dn tag = piece.b();
                        aqe bb=piece.c();
                        out.printf("%d %d %s %d %d %d %d %d %d %d %d %d %d\n",cx,cz,
                            tag.j("id"),tag.f("O"),piece.d(),bb.a,bb.b,bb.c,bb.d,bb.e,bb.f,
                            tag.n("Chest") ? 1 : 0,tag.f("Seed"));
                    }
                }
            }
            return;
        }
        if (args[3].equals("end-island")) {
            EndWorld end = new EndWorld(seed);
            for (int cz=-8;cz<8;cz++) for (int cx=-8;cx<8;cx++) end.provider.a(end.provider,cx,cz);
            for(String entity:end.entities) System.out.println(entity);
            try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[4])))) {
                for (int cz=-8;cz<8;cz++) for (int cx=-8;cx<8;cx++) {
                    ans col=end.column(cx,cz);
                    for (int y=0;y<256;y++) for(int dz=0;dz<16;dz++) for(int dx=0;dx<16;dx++)
                        out.writeByte(afh.a(col.a(dx,y,dz).c()));
                }
            }
            return;
        }
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
