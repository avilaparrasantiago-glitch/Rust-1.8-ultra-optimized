import java.util.*;
class Material { static final Material air = new Material(); }
class Block {
    final int id;
    Block(int id) { this.id=id; }
    Material getMaterial() {return id==0 ? Material.air : null;}
    Block getDefaultState(){return this;}
    Block getBlock(){return this;}
}
class Blocks {
    static final Block air=new Block(0), dirt=new Block(3), snow=new Block(80),
        ice=new Block(79), packed_ice=new Block(174), stone=new Block(1);
}
class BlockPos {
    final int x,y,z;
    BlockPos(int x,int y,int z){this.x=x;this.y=y;this.z=z;}
    int getX(){return x;} int getY(){return y;} int getZ(){return z;}
    BlockPos add(int a,int b,int c){return new BlockPos(x+a,y+b,z+c);}
    BlockPos up(int n){return add(0,n,0);} BlockPos down(int n){return up(-n);}
    BlockPos down(){return down(1);}
}
class World {
    Map<String,Block> map=new HashMap<>();
    Block getBlockState(BlockPos p){return map.getOrDefault(p.x+","+p.y+","+p.z,
        p.y>64 ? Blocks.air : p.y==64 ? Blocks.snow : p.y>=52 ? Blocks.dirt : Blocks.stone);}
    boolean isAirBlock(BlockPos p){return getBlockState(p)==Blocks.air;}
    void setBlockState(BlockPos p,Block b,int flags){map.put(p.x+","+p.y+","+p.z,b);}
}
class WorldGenerator {
    void setBlockAndNotifyAdequately(World w,BlockPos p,Block b){w.setBlockState(p,b,2);}
}
class MathHelper {
    static int ceiling_float_int(float f){return (int)Math.ceil(f);}
    static int abs_int(int n){return Math.abs(n);}
}
public class IceVector {
    public static void main(String[] args){
        for(long seed : new long[]{12345, 0, 95}){
            World w=new World(); Random r=new Random(seed);
            for(int i=0;i<5;i++){
                int x=-8+r.nextInt(16), z=-8+r.nextInt(16), y=255;
                while(w.isAirBlock(new BlockPos(x,y,z)))y--;
                BlockPos p=new BlockPos(x,y+1,z);
                if(i<3)new WorldGenIceSpike().generate(w,r,p);
                else new WorldGenIcePath(4).generate(w,r,p);
            }
            long h=0xcbf29ce484222325L;
            for(int x=-16;x<16;x++)for(int z=-16;z<16;z++)for(int y=50;y<128;y++)
                h=(h^(w.getBlockState(new BlockPos(x,y,z)).id<<4))*0x100000001b3L;
            System.out.printf("%d %016x %d%n",seed,h,r.nextLong());
        }
    }
}
