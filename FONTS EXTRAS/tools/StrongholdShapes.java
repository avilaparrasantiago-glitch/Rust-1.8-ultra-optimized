import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.lang.reflect.Field;
import java.util.zip.GZIPOutputStream;

public final class StrongholdShapes {
    static final String[] kinds={"n","h","d","i","j","o","l","c","a","e","b"};
    static final String[] doors={"OPENING","WOOD_DOOR","GRATES","IRON_DOOR"};
    static final List<int[]> ops=new ArrayList<>();
    public static void op(int... values){ops.add(values);}
    public static int state(alz s){return afh.a(s.c())*16+s.c().c(s);}
    static String overrides() {
        return "protected boolean a(adm w,aqe b){return false;}\n"+
        "protected void a(adm w,alz s,int x,int y,int z,aqe b){StrongholdShapes.op(1,x,y,z,StrongholdShapes.state(s));}\n"+
        "protected void a(adm w,aqe b,int x,int y,int z,int u,int v,int t,alz s,alz i,boolean existing){StrongholdShapes.op(2,x,y,z,u,v,t,StrongholdShapes.state(s),StrongholdShapes.state(i),existing?1:0);}\n"+
        "protected void a(adm w,aqe b,int x,int y,int z,int u,int v,int t,boolean e,java.util.Random r,aqt.a sel){StrongholdShapes.op(3,x,y,z,u,v,t,e?1:0);}\n"+
        "protected void a(adm w,aqe b,java.util.Random r,float p,int x,int y,int z,int u,int v,int t,alz s,alz i,boolean e){StrongholdShapes.op(4,x,y,z,u,v,t,StrongholdShapes.state(s),StrongholdShapes.state(i),e?1:0,Math.round(p*100));}\n"+
        "protected void a(adm w,aqe b,java.util.Random r,float p,int x,int y,int z,alz s){StrongholdShapes.op(5,x,y,z,StrongholdShapes.state(s),Math.round(p*100));}\n"+
        "protected boolean a(adm w,aqe b,java.util.Random r,int x,int y,int z,java.util.List<ob> pool,int rolls){StrongholdShapes.op(6,x,y,z);return true;}\n";
    }
    static void pool(String cls,String field)throws Exception{
        Field list=Class.forName(cls).getDeclaredField(field);list.setAccessible(true);
        for(Object item:(List<?>)list.get(null)){
            Field stack=ob.class.getDeclaredField("b"),min=ob.class.getDeclaredField("c"),max=ob.class.getDeclaredField("d");
            stack.setAccessible(true);min.setAccessible(true);max.setAccessible(true);
            dn tag=new dn();((zx)stack.get(item)).b(tag);
            System.out.printf("%s %s %d %d %d %d%n",cls,tag.j("id"),tag.e("Damage"),min.getInt(item),max.getInt(item),((ob)item).a);
        }
    }
    public static void main(String[] args)throws Exception{
        if(args[0].equals("source")){
            StringBuilder out=new StringBuilder("class ShapeProbes {\n");
            for(int n=0;n<kinds.length;n++)out.append("static class P"+n+" extends aqp."+kinds[n]+"{\n"+overrides()+"}\n");
            Files.write(Paths.get(args[1]),out.append("}\n").toString().getBytes("UTF-8"));return;
        }
        kb.c();pool("aqp$a","a");pool("aqp$j","b");
        TerrainReference.World world=new TerrainReference.World(12345);
        try(DataOutputStream out=new DataOutputStream(new GZIPOutputStream(new FileOutputStream(args[1])))){
            for(int kind=0;kind<kinds.length;kind++)for(int facing=0;facing<4;facing++)for(int door=0;door<4;door++){
                int variants=kind==0?4:kind==4?5:kind==7?16:kind==9?2:kind==10?4:1;
                for(int variant=0;variant<variants;variant++){
                    dn tag=new dn();tag.a("O",facing);tag.a("GD",3);tag.a("BB",new int[]{0,32,0,20,42,20});
                    tag.a("EntryDoor",doors[door]);tag.a("Tall",variant!=0);tag.a("Type",variant);
                    tag.a("Steps",variant+1);tag.a("Left",(variant&1)!=0);tag.a("Right",(variant&2)!=0);
                    String[] flags={"leftLow","leftHigh","rightLow","rightHigh"};
                    for(int i=0;i<4;i++)tag.a(flags[i],(variant&(1<<i))!=0);
                    aqt piece=(aqt)Class.forName("ShapeProbes$P"+kind).newInstance();
                    piece.a(world,tag);ops.clear();piece.a(world,new Random(0),new aqe(-128,0,-128,127,255,127));
                    out.writeInt((kind<<12)|(facing<<10)|(door<<8)|variant);out.writeShort(ops.size());
                    for(int[] op:ops){out.writeByte(op.length);for(int v:op)out.writeShort(v);}
                }
            }
        }
    }
}
