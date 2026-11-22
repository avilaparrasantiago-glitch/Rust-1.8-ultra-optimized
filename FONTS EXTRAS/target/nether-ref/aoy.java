/*
 * Decompiled with CFR 0.152.
 */
import java.util.Random;

public class aoy
extends aot {
    private final afh a;
    private final boolean b;

    public aoy(afh afh2, boolean bl) {
        this.a = afh2;
        this.b = bl;
    }

    @Override
    public boolean b(adm adm2, Random random, cj cj2) {
        if (adm2.p(cj2.a()).c() != afi.aV) {
            return false;
        }
        if (adm2.p(cj2).c().t() != arm.a && adm2.p(cj2).c() != afi.aV) {
            return false;
        }
        int n = 0;
        if (adm2.p(cj2.e()).c() == afi.aV) {
            ++n;
        }
        if (adm2.p(cj2.f()).c() == afi.aV) {
            ++n;
        }
        if (adm2.p(cj2.c()).c() == afi.aV) {
            ++n;
        }
        if (adm2.p(cj2.d()).c() == afi.aV) {
            ++n;
        }
        if (adm2.p(cj2.b()).c() == afi.aV) {
            ++n;
        }
        cfr_renamed_0 = 0;
        if (adm2.d(cj2.e())) {
            ++cfr_renamed_0;
        }
        if (adm2.d(cj2.f())) {
            ++cfr_renamed_0;
        }
        if (adm2.d(cj2.c())) {
            ++cfr_renamed_0;
        }
        if (adm2.d(cj2.d())) {
            ++cfr_renamed_0;
        }
        if (adm2.d(cj2.b())) {
            ++cfr_renamed_0;
        }
        if (!this.b && n == 4 && cfr_renamed_0 == 1 || n == 5) {
            adm2.a(cj2, this.a.Q(), 2);
            adm2.a(this.a, cj2, random);
        }
        return true;
    }
}
