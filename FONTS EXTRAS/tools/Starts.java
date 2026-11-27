import java.io.*;
import java.util.*;
class Starts {
 public static void main(String[] args) throws Exception {
  kb.c(); long seed=12345; TerrainReference.World world=new TerrainReference.World(seed){public ady b(cj pos){return v().a(pos);} };
  Set<String> seen=new HashSet<>();
  for(aqq g:new aqq[]{new aqm(),new aqk()}) {
   g.c=world;
   for(int z=-512;z<512;z++) for(int x=-512;x<512;x++) {
    if(!g.a(x,z)) continue;
    Random rand=new Random(seed); g.b.setSeed(seed ^ x*rand.nextLong() ^ z*rand.nextLong()); g.b.nextInt();
    aqu start=g.b(x,z); String piece=start.b().getFirst().getClass().getName();
    if(seen.add(piece)) {
      dn tag=start.a(x,z);System.out.println(tag);
      try(FileOutputStream out=new FileOutputStream(new File(args[0],piece+".nbt.gz"))) {dx.a(tag,out);}
    }
   }
  }
 }
}
