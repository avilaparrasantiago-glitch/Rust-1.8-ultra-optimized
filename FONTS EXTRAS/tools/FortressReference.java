import java.io.*;
import java.util.*;

// Export local block shapes from individual vanilla pieces. The Rust generator
// selects, orients and positions pieces, and extends their foundations into terrain.
public final class FortressReference {
    static final String[] ids={"NeBS","NeBCr","NeRC","NeSR","NeMT","NeCE","NeSC","NeSCSC","NeSCRT","NeSCLT","NeCCS","NeCTB","NeCSR"};
    static class World extends TerrainReference.World {
        // 0 is untouched, 1 is an explicit air write, other values are state + 1.
        final int[][][] writes = new int[32][32][32];
        final int lower, floor;
        World(long seed) {this(seed,63,62);}
        World(long seed,int lower,int floor) {super(seed);this.lower=lower;this.floor=floor;}
        public alz p(cj pos) {
            int y=pos.o()-lower;
            if(y<0||y>=32||pos.n()<0||pos.n()>=32||pos.p()<0||pos.p()>=32)
                return pos.o()<=floor ? afi.aV.Q() : afi.a.Q();
            int state=writes[y][pos.p()][pos.n()]-1;
            return state<0 ? (pos.o()<=floor ? afi.aV.Q() : afi.a.Q()) : afh.d((state>>4)|((state&15)<<12));
        }
        public boolean a(cj pos, alz state,int flags) {
            writes[pos.o()-lower][pos.p()][pos.n()]=1+(afh.a(state.c())<<4)+state.c().c(state);
            return true;
        }
        public akw s(cj pos) {return null;}
        public void a(afh block,cj pos,Random random) {}
        void rectangles(DataOutputStream out) throws IOException {
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();
            DataOutputStream records=new DataOutputStream(bytes);
            for(int y=0;y<32;y++) for(int z=0;z<32;z++) for(int x=0;x<32;x++) {
                int v=writes[y][z][x];
                if(v==0) continue;
                int xx=x,zz=z,yy=y;
                while(xx+1<32 && writes[y][z][xx+1]==v) xx++;
                rows:while(zz+1<32) {for(int i=x;i<=xx;i++) if(writes[y][zz+1][i]!=v) break rows;zz++;}
                levels:while(y>0 && yy+1<32) {
                    for(int j=z;j<=zz;j++) for(int i=x;i<=xx;i++) if(writes[yy+1][j][i]!=v) break levels;
                    yy++;
                }
                records.writeByte(x);records.writeByte(y+lower-64);records.writeByte(z);
                records.writeByte(xx);records.writeByte(yy+lower-64);records.writeByte(zz);
                records.writeShort(v-1);
                for(int k=y;k<=yy;k++)for(int j=z;j<=zz;j++)Arrays.fill(writes[k][j],x,xx+1,0);
            }
            out.writeShort(bytes.size()/8);bytes.writeTo(out);
        }
    }
    public static void main(String[] args) throws Exception {
        kb.c();
        if(args.length>1 && args[1].equals("desert")) {
            try(DataOutputStream out=new DataOutputStream(new FileOutputStream(args[0]))) {
                for(int facing=0;facing<4;facing++){
                    World w=new World(12345,48,59);
                    aqn.a piece=new aqn.a(new Random(12345),0,0);
                    dn tag=piece.b();tag.a("O",facing);piece.a(w,tag);
                    piece.a(w,new Random(12345),new aqe(-32,0,-32,64,255,64));
                    w.rectangles(out);
                }
            }
            return;
        }
        Map<String,aqt> representatives=new HashMap<>();
        TerrainReference.World base=new TerrainReference.World(12345);
        aqi generator=new aqi();generator.c=base;
        for(int z=-64;z<64;z++)for(int x=-64;x<64;x++){
            if(generator.a(x,z)) for(aqt p:generator.b(x,z).b()) representatives.putIfAbsent(p.b().j("id"),p);
        }
        try(DataOutputStream out=new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[0])))){
            for(String id:ids)for(int facing=0;facing<4;facing++){
                World w=new World(12345);
                aqt example=representatives.get(id);
                dn tag=example.b();aqe b=example.c();
                int width=b.d-b.a,length=b.f-b.c;
                if(((tag.f("O")^facing)&1)!=0){int t=width;width=length;length=t;}
                tag.a("BB",new int[]{0,64,0,width,64+b.e-b.b,length});
                tag.a("O",facing);tag.a("Chest",false);tag.a("Mob",false);
                aqt p=example.getClass().newInstance();
                p.a(w,tag);
                p.a(w,new Random(12345),new aqe(-32,0,-32,64,255,64));
                w.rectangles(out);
            }
        }
    }
}
