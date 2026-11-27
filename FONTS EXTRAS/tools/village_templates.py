"""Capture the ordered village building operations from the user's server JAR.

Run with the JDK bin directory on PATH. Each adapter intercepts the same
structure helper calls while inheriting the actual vanilla piece generator.
"""
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
jar = root / "target/tmp/server-1.8.9.jar"
output = root / "vectors/T2.4b-4/village-buildings.bin"
kinds = "giahfb cjm dp".replace(" ", "")

adapter = """
static class PCLASS extends aqw.CLASS {
    void op(int x,int y,int z,int code,int state) {
        emit(a(x,z),y,b(x,z),code,state);
    }
    protected void a(adm w,alz s,int x,int y,int z,aqe box) {
        alz block=a(s);
        int id=afh.a(block.c()),state=id*16+block.c().c(block);
        int code=(id==59 || id==141 || id==142 || id==104) ? 5 : 0;
        if (code==5) state=(x-1)/3;
        if (z==-1 && y==0) code=3;
        op(x,y,z,code,state);
    }
    protected alz a(adm w,int x,int y,int z,aqe box){return y<0?afi.b.Q():afi.a.Q();}
    protected void b(adm w,int x,int y,int z,aqe box){op(x,y,z,1,0);}
    protected void b(adm w,alz s,int x,int y,int z,aqe box){
        alz block=a(s);op(x,y,z,2,afh.a(block.c())*16+block.c().c(block));
    }
    protected void a(adm w,aqe box,Random r,int x,int y,int z,cq facing){
        op(x,y,z,4,facing.b());
    }
    protected boolean a(adm w,aqe box,Random r,int x,int y,int z,List<ob> pool,int rolls){
        op(x,y,z,6,0);return true;
    }
    protected void a(adm w,aqe box,int x,int y,int z,int count){
        for(int i=0;i<count;i++)op(x+i,y,z,7,(i<<8)|(c(i,5)&255));
    }
}
"""
source = """
import java.io.*;
import java.util.*;
class CaptureVillage {
    static DataOutputStream out;
    static void emit(int x,int y,int z,int op,int state){
        try {out.writeByte(x);out.writeByte(y);out.writeByte(z);out.writeByte(op);out.writeShort(state);}
        catch(IOException e){throw new UncheckedIOException(e);}
    }
    ADAPTERS
    public static void main(String[] args)throws Exception {
        kb.c();
        int[][] sizes={{5,5},{5,9},{9,6},{4,5},{9,11},{13,9},{7,9},{10,7},{9,12},{3,2},{6,6}};
        try(DataOutputStream file=new DataOutputStream(new FileOutputStream(args[0]))){
            for(int k=0;k<sizes.length;k++)for(int desert=0;desert<2;desert++)
            for(int dir=0;dir<4;dir++)for(int variant=0;variant<(k==0?2:k==3?6:1);variant++){
                aqw.n piece=new aqw.n[]{INSTANCES}[k];
                piece.m=cq.b(dir);
                int w=sizes[k][dir%2],l=sizes[k][1-dir%2];
                piece.l=new aqe(0,64,0,w-1,79,l-1);
                dn tag=new dn();tag.a("HPos",64);tag.a("Desert",desert!=0);
                tag.a("Terrace",variant!=0);tag.a("C",variant>=3);tag.a("T",variant%3);
                for(String crop:new String[]{"CA","CB","CC","CD"})tag.a(crop,59);
                piece.b(tag);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                out=new DataOutputStream(bytes);
                piece.a(null,new Random(1),new aqe(-32,0,-32,32,255,32));
                file.writeByte(k);file.writeByte(desert);file.writeByte(dir);file.writeByte(variant);
                file.writeInt(bytes.size());file.write(bytes.toByteArray());
            }
        }
    }
}
"""
source = source.replace("ADAPTERS", "".join(adapter.replace("CLASS", k) for k in kinds))
source = source.replace("INSTANCES", ",".join(f"new P{k}()" for k in kinds))
with tempfile.TemporaryDirectory() as folder:
    path = Path(folder, "CaptureVillage.java")
    path.write_text(source, encoding="utf-8")
    subprocess.run(["javac", "-cp", str(jar), str(path)], check=True)
    subprocess.run(["java", "-cp", os.pathsep.join([folder, str(jar)]),
                    "CaptureVillage", str(output)], check=True)
