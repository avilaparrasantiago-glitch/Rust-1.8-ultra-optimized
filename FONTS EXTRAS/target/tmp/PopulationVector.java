import java.util.*;
public class PopulationVector extends TerrainReference.DesertWorld {
    static java.io.File neighborsFile() {
        java.io.File vector = new java.io.File("vectors/T2.4b-1/dungeon-neighbors.nbt");
        return vector.isFile() ? vector : new java.io.File("FONTS EXTRAS", vector.getPath());
    }
    final Map<Long,akw> tiles=new HashMap<>();
    PopulationVector()throws Exception{super(12345);}
    public akw s(cj p) {
        int id=afh.a(p(p).c());
        if(id!=52&&id!=54)return null;
        akw tile=tiles.computeIfAbsent(p.g(),k->id==54 ? new aky() : new all()); tile.a(new cj(p.n(),p.o(),p.p())); return tile;
    }
    public boolean a(cj p,alz state,int flags){
        if(p.o()<0||p.o()>255)return false;
        if(state.c()!=p(p).c())tiles.remove(p.g());
        return super.a(p,state,flags);
    }
    public cj m(cj p){int y=255;while(y>=0&&d(new cj(p.n(),y,p.p())))y--;return new cj(p.n(),y+1,p.p());}
    public cj r(cj p){int y=255;while(y>=0){arm mat=p(new cj(p.n(),y,p.p())).c().t();if(mat.c()&&mat!=arm.j)break;y--;}return new cj(p.n(),y+1,p.p());}
    public int l(cj p){return 0;} public int k(cj p){return 0;}
    public int c(cj p,boolean b){return 0;} public int b(ads channel,cj p){return 0;}
    public void a(afh b,cj p,Random r){}
    public boolean v(cj p){return false;}
    void run(int cx,int cz) {
        Random r=new Random(12345);
        long a=r.nextLong()/2*2+1, b=r.nextLong()/2*2+1;
        r.setSeed((cx*a+cz*b)^12345);
        cj base=new cj(cx*16,0,cz*16);
        ady biome=b(base.a(16,0,16));
        if(biome.az!=2 && biome.az!=17 && r.nextInt(4)==0)new apc(afh.c(9)).b(this,r,base.a(8+r.nextInt(16),r.nextInt(256),8+r.nextInt(16)));
        if(r.nextInt(8)==0){
            cj pos=base.a(8+r.nextInt(16),r.nextInt(r.nextInt(248)+8),8+r.nextInt(16));
            if(pos.o()<63||r.nextInt(10)==0)new apc(afh.c(11)).b(this,r,pos);
        }
        for(int i=0;i<8;i++)new api().b(this,r,base.a(8+r.nextInt(16),r.nextInt(256),8+r.nextInt(16)));
        biome.a(this,r,base);
        java.io.ByteArrayOutputStream bytes=new java.io.ByteArrayOutputStream(); long h=0xcbf29ce484222325L;
        for(int x0=cx;x0<=cx+1;x0++)for(int z0=cz;z0<=cz+1;z0++)
            for(int y=0;y<256;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++){
                alz s=p(new cj(x0*16+x,y,z0*16+z));afh block=s.c();
                int packed=(afh.a(block)<<4)|block.c(s); bytes.write(packed);bytes.write(packed>>8);h=(h^((afh.a(block)<<4)|block.c(s)))*0x100000001b3L;
            }
        try {java.nio.file.Files.write(java.nio.file.Paths.get("target/tmp/java-pop.bin"),bytes.toByteArray());}catch(Exception e){throw new RuntimeException(e);}java.util.List<akw> sorted=new ArrayList<>(tiles.values());sorted.sort(Comparator.comparingInt((akw t)->t.v().n()).thenComparingInt(t->t.v().o()).thenComparingInt(t->t.v().p()));du entries=new du();for(akw tile:sorted){dn tag=new dn();tile.b(tag);entries.a(tag);}dn root=new dn();root.a("TileEntities",entries);try(java.io.DataInputStream input=new java.io.DataInputStream(new java.io.FileInputStream(neighborsFile()))){if(!root.equals(dx.a(input)))throw new AssertionError(root.toString());}catch(java.io.IOException e){throw new RuntimeException(e);}System.out.printf("%d %016x %d %d%n",biome.az,h,r.nextLong(),tiles.size());
    }
    public static void main(String[] args)throws Exception{kb.c();new PopulationVector().run(-2,-3);}
}
