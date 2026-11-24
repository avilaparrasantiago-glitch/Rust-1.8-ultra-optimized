import java.util.*;
public class BiomeVectors extends TerrainReference.World {
    final Map<Long,alz> blocks=new HashMap<>();
    final ady biome;
    BiomeVectors(int id){super(12345);biome=ady.e(id);}
    public ady b(cj pos){return biome;}
    public alz p(cj pos){
        int y=pos.o(), id=y>4 ? 0 : y==4 ? 2 : y>=2 ? 3 : 1;
        return blocks.getOrDefault(pos.g(),afh.c(id).Q());
    }
    public boolean a(cj p,alz s,int flags){if(p.o()<0||p.o()>=256)return false;blocks.put(p.g(),s);return true;}
    public boolean d(cj p){return p(p).c()==afi.a;}
    public cj m(cj p){int y=255;while(y>=0&&d(new cj(p.n(),y,p.p())))y--;return new cj(p.n(),y+1,p.p());}
    public cj r(cj p){return m(p);}
    public int l(cj p){return 15;}
    public int k(cj p){return 15;}
    public int c(cj p,boolean b){return 15;}
    public void a(afh b,cj p,Random r){throw new AssertionError();}
    public static void main(String[] args){
        kb.c();
        for(String arg:args){
            int id=Integer.parseInt(arg);
            BiomeVectors w=new BiomeVectors(id);
            Random r=new Random(12345);w.biome.a(w,r,new cj(-16,0,-16));
            long h=0xcbf29ce484222325L;
            for(int x=-16;x<16;x++)for(int z=-16;z<16;z++)for(int y=0;y<256;y++){
                alz s=w.p(new cj(x,y,z));afh b=s.c();
                h=(h^((afh.a(b)<<4)|b.c(s)))*0x100000001b3L;
            }
            System.out.printf("%d %016x %d%n",id,h,r.nextLong());
        }
    }
}
