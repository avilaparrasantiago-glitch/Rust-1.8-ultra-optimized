/*
 * Decompiled with CFR 0.152.
 */
import java.util.Iterator;
import java.util.LinkedList;
import java.util.Random;

public abstract class aqu {
    protected LinkedList<aqt> a = new LinkedList();
    protected aqe b;
    private int c;
    private int d;

    public aqu() {
    }

    public aqu(int n, int n2) {
        this.c = n;
        this.d = n2;
    }

    public aqe a() {
        return this.b;
    }

    public LinkedList<aqt> b() {
        return this.a;
    }

    public void a(adm adm2, Random random, aqe aqe2) {
        Iterator iterator = this.a.iterator();
        while (iterator.hasNext()) {
            aqt aqt2 = (aqt)iterator.next();
            if (!aqt2.c().a(aqe2) || aqt2.a(adm2, random, aqe2)) continue;
            iterator.remove();
        }
    }

    protected void c() {
        this.b = aqe.a();
        for (aqt aqt2 : this.a) {
            this.b.b(aqt2.c());
        }
    }

    public dn a(int n, int n2) {
        dn dn2 = new dn();
        dn2.a("id", aqr.a(this));
        dn2.a("ChunkX", n);
        dn2.a("ChunkZ", n2);
        dn2.a("BB", this.b.g());
        du cfr_renamed_0 = new du();
        for (aqt aqt2 : this.a) {
            cfr_renamed_0.a(aqt2.b());
        }
        dn2.a("Children", cfr_renamed_0);
        this.a(dn2);
        return dn2;
    }

    public void a(dn dn2) {
    }

    public void a(adm adm2, dn dn22) {
        dn dn22;
        this.c = dn22.f("ChunkX");
        this.d = dn22.f("ChunkZ");
        if (dn22.c("BB")) {
            this.b = new aqe(dn22.l("BB"));
        }
        du du2 = dn22.c("Children", 10);
        for (int i = 0; i < du2.c(); ++i) {
            this.a.add(aqr.b(du2.b(i), adm2));
        }
        this.b(dn22);
    }

    public void b(dn dn2) {
    }

    protected void a(adm adm2, Random random, int n) {
        cfr_renamed_1 = adm2.F() - n;
        cfr_renamed_1 = this.b.d() + 1;
        if (cfr_renamed_1 < cfr_renamed_1) {
            cfr_renamed_1 += random.nextInt(cfr_renamed_1 - cfr_renamed_1);
        }
        cfr_renamed_1 = cfr_renamed_1 - this.b.e;
        this.b.a(0, cfr_renamed_1, 0);
        for (aqt aqt2 : this.a) {
            aqt2.a(0, cfr_renamed_1, 0);
        }
    }

    protected void a(adm adm2, Random random, int n, int n2) {
        cfr_renamed_1 = n2 - n + 1 - this.b.d();
        cfr_renamed_1 = 1;
        cfr_renamed_1 = cfr_renamed_1 > 1 ? n + random.nextInt(cfr_renamed_1) : n;
        cfr_renamed_1 = cfr_renamed_1 - this.b.b;
        this.b.a(0, cfr_renamed_1, 0);
        for (aqt aqt2 : this.a) {
            aqt2.a(0, cfr_renamed_1, 0);
        }
    }

    public boolean d() {
        return true;
    }

    public boolean a(adg adg2) {
        return true;
    }

    public void b(adg adg2) {
    }

    public int e() {
        return this.c;
    }

    public int f() {
        return this.d;
    }
}
