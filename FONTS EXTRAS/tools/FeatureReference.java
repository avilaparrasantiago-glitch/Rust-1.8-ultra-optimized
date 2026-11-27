import java.io.*;
import java.util.*;
import java.security.MessageDigest;

// Dedicated 1.8.9 structure population with a reproducible substrate and owner order.
public final class FeatureReference {
    static class World extends TerrainReference.World {
        final boolean ocean;
        final Map<cj,alz> blocks=new HashMap<>();
        final Map<cj,akw> tiles=new HashMap<>();
        final List<pk> spawned=new ArrayList<>();
        World(boolean ocean,long seed)throws Exception {super(seed);this.ocean=ocean;
            java.lang.reflect.Field world=anm.class.getDeclaredField("b");world.setAccessible(true);world.set(t,this);
        }
        public int F(){return 63;}
        public ok E(cj pos){return new ok(oj.c,0,0,0);}
        public cj r(cj pos){return new cj(pos.n(),64,pos.p());}
        public boolean d(pk entity){spawned.add(entity);return true;}
        public akw s(cj p){return tiles.get(p);}
        public alz p(cj p){
            alz fallback=p.o()<(ocean?32:64)?afi.b.Q():ocean&&p.o()<63?afi.j.Q():afi.a.Q();
            return blocks.getOrDefault(p,fallback);
        }
        public boolean a(cj p,alz state,int flags){
            if(p(p).c()!=state.c()){
                tiles.remove(p);
                akw tile=state.c()==afi.ae?new aky():state.c()==afi.z?new alc():null;
                if(tile!=null){tile.a(p);tiles.put(p,tile);}
            }
            blocks.put(new cj(p.n(),p.o(),p.p()),state);return true;
        }
    }
    public static void main(String[] args)throws Exception{
        kb.c();
        for(long seed:new long[]{12345,-9581}) for(int[] coords:new int[][]{{111,-211},{146,-509},{-249,-508}}){
            int x=coords[0],z=coords[1];boolean ocean=x==-249;
            boolean jungle=x==111; World w=new World(ocean,seed);aqq g=ocean?new aqk():new aqm();g.c=w;
            if(seed!=12345){
                search: for(int cz=-320;cz<=320;cz++)for(int cx=-320;cx<=320;cx++){
                    if(!g.a(cx,cz))continue;
                    int biome=w.b(new cj(cx*16+8,0,cz*16+8)).az;
                    if(!ocean && !(jungle?(biome==21||biome==22):biome==6))continue;
                    x=cx;z=cz;break search;
                }
            }
            Random r=new Random(seed);g.b.setSeed(seed^x*r.nextLong()^z*r.nextLong());g.b.nextInt();
            if(!g.a(x,z))throw new AssertionError("not a viable start");
            aqu start=g.b(x,z);aqe bounds=start.a();dn root=new dn(),tiles=new dn(),streams=new dn();
            for(int cx=(bounds.a-8)>>4;cx<=((bounds.d-8)>>4);cx++)for(int cz=(bounds.c-8)>>4;cz<=((bounds.f-8)>>4);cz++){
                r.setSeed(seed); r.setSeed((cx*(r.nextLong()/2*2+1)+cz*(r.nextLong()/2*2+1))^seed);
                start.a(w,r,new aqe(cx*16+8,0,cz*16+8,cx*16+23,255,cz*16+23));
                streams.a(cx+","+cz,r.nextInt());
            }
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
            for(int y=30;y<85;y++)for(int zz=bounds.c-8;zz<=bounds.f+8;zz++)for(int xx=bounds.a-8;xx<=bounds.d+8;xx++){
                alz state=w.p(new cj(xx,y,zz));out.writeShort(afh.a(state.c())*16+state.c().c(state));
            }
            root.a("States",bytes.toByteArray());root.a("Next",streams);
            for(Map.Entry<cj,akw> e:w.tiles.entrySet()){dn tag=new dn();e.getValue().b(tag);cj p=e.getKey();tiles.a(p.n()+","+p.o()+","+p.p(),tag);}
            root.a("Tiles",tiles);du entities=new du();
            for(pk e:w.spawned){if(seed==12345 && entities.c()==0) {
                dn snapshot=new dn();e.d(snapshot);
                for(String transientKey:new String[]{"UUIDMost","UUIDLeast","Pos"}) snapshot.o(transientKey);
                du attributes=snapshot.c("Attributes",10);
                for(int i=0;i<attributes.c();i++){
                    du modifiers=attributes.b(i).c("Modifiers",10);
                    for(int k=0;k<modifiers.c();k++){
                        dn modifier=modifiers.b(k);modifier.a("UUIDMost",0L);modifier.a("UUIDLeast",0L);modifier.a("Amount",0.0);
                    }
                }
                try(FileOutputStream outEntity=new FileOutputStream(new File(args[0],ocean?"elder.nbt.gz":"witch.nbt.gz"))){dx.a(snapshot,outEntity);}
            }
            dn tag=new dn();tag.a("X",(int)Math.floor(e.s));tag.a("Y",(int)Math.floor(e.t));tag.a("Z",(int)Math.floor(e.u));entities.a(tag);}
            root.a("Entities",entities);
            String name=ocean?"monument":jungle?"jungle":"hut";
            System.out.printf("%d %s %d %d %d%n",seed,name,x,z,start.b().getFirst().m.b());
            if(seed!=12345)name+="-9581";
            try(FileOutputStream output=new FileOutputStream(new File(args[0],name+"-population.nbt.gz"))){dx.a(root,output);}
        }
    }
}
