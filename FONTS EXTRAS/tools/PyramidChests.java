import java.util.*;
import java.io.*;
class PyramidChests {
 public static void main(String[] args)throws Exception{
  kb.c(); final Map<cj,aky> tiles=new HashMap<>();
  TerrainReference.DesertWorld world=new TerrainReference.DesertWorld(12345){
   public boolean a(cj p,alz state,int flags){
    boolean changed=super.a(p,state,flags);
    if(state.c()==afi.ae && !tiles.containsKey(p)){aky t=new aky();t.a(p);tiles.put(p,t);}
    return changed;
   }
   public akw s(cj p){return tiles.get(p);}
  };
  int x=-191,z=-493; Random r=new Random(12345); aqm g=new aqm();g.c=world;
  g.b.setSeed(12345^x*r.nextLong()^z*r.nextLong());g.b.nextInt();
  aqu start=g.b(x,z); r.setSeed(12345);r.setSeed((x*(r.nextLong()/2*2+1)+z*(r.nextLong()/2*2+1))^12345);
  start.a(world,r,new aqe(x*16+8,0,z*16+8,x*16+23,255,z*16+23));
  dn root=new dn();
  for(Map.Entry<cj,aky> e:tiles.entrySet()){
   cj p=e.getKey();dn tag=new dn();e.getValue().b(tag);root.a(p.n()+","+p.o()+","+p.p(),tag);
  }
  root.a("Next",r.nextInt());System.out.println(root);try(FileOutputStream out=new FileOutputStream(args[0])){dx.a(root,out);}
 }
}
