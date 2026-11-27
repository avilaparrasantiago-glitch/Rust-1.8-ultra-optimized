import java.io.*;
import java.util.*;

// Replay the official piece placement calls on flat ground. Each five-byte record
// is a rotated offset and a state, or a terrain/RNG-dependent operation.
final class ScatteredTemplates {
    static DataOutputStream out;
    static final int STONE=0xff00, SUPPORT=0xff01, CHEST=0xff02, TRAP=0xff04;
    static void emit(int x,int y,int z,int state) {
        try {out.writeByte(x);out.writeByte(y-64);out.writeByte(z);out.writeShort(state);}
        catch(IOException e){throw new UncheckedIOException(e);}
    }
    static class Jungle extends aqn.b {
        int chest,trap;
        Jungle(Random r){super(r,0,0);d=64;}
        protected void a(adm w,alz s,int x,int y,int z,aqe bb){
            emit(a(x,z),d(y),b(x,z),afh.a(s.c())*16+s.c().c(s));
        }
        protected void a(adm w,aqe bb,int x0,int y0,int z0,int x1,int y1,int z1,
                         boolean existing,Random r,aqt.a selector){
            if(existing) throw new AssertionError();
            for(int y=y0;y<=y1;y++) for(int x=x0;x<=x1;x++) for(int z=z0;z<=z1;z++)
                emit(a(x,z),d(y),b(x,z),STONE);
        }
        protected boolean a(adm w,aqe bb,Random r,int x,int y,int z,List<ob> pool,int n){
            emit(a(x,z),d(y),b(x,z),CHEST+chest++);return true;
        }
        protected boolean a(adm w,aqe bb,Random r,int x,int y,int z,int facing,List<ob> pool,int n){
            emit(a(x,z),d(y),b(x,z),TRAP+trap++*16+a(afi.z,facing));return true;
        }
    }
    static class Hut extends aqn.d {
        Hut(Random r){super(r,0,0);d=64;dn tag=new dn();super.a(tag);tag.a("Witch",true);b(tag);}
        protected void a(adm w,alz s,int x,int y,int z,aqe bb){
            emit(a(x,z),d(y),b(x,z),afh.a(s.c())*16+s.c().c(s));
        }
        protected void b(adm w,alz s,int x,int y,int z,aqe bb){
            emit(a(x,z),d(y),b(x,z),SUPPORT);
        }
    }
    public static void main(String[] args)throws Exception{
        kb.c(); Random r=new Random(12345);
        for(String kind:new String[]{"hut","jungle"}){
            Set<Integer> seen=new HashSet<>();
            try(DataOutputStream file=new DataOutputStream(new FileOutputStream(new File(args[0],kind+".bin")))){
                byte[][] shapes=new byte[4][];
                while(seen.size()<4){
                    aqt piece=kind.equals("hut")?new Hut(r):new Jungle(r);
                    int dir=piece.m.b();if(!seen.add(dir)) continue;
                    ByteArrayOutputStream bytes=new ByteArrayOutputStream();out=new DataOutputStream(bytes);
                    piece.a(null,r,new aqe(-32,0,-32,32,255,32));shapes[dir]=bytes.toByteArray();
                }
                for(byte[] shape:shapes){file.writeInt(shape.length);file.write(shape);}
            }
        }
    }
}
