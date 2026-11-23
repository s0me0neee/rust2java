class Main {
    // A single `&mut` primitive parameter becomes a return value: `a = bump(a);`.
    static final class Stats {
        int hits;

        public Stats(int hits) {
            this.hits = hits;
        }
    }

    static int bump(int n) {
        n += 1;
        return n;
    }

    static long clamp_to(long v, long max) {
        if (v > max) {
            v = max;
            return v;
        }
        v += 1L;
        return v;
    }

    public static void main(String[] args) {
        int a = 1;
        a = bump(a);
        a = bump(a);

        Stats stats = new Stats(0);
        stats.hits = bump(stats.hits);

        long big = 10L;
        big = clamp_to(big, 5L);
        big = clamp_to(big, 100L);
        System.out.println(a + " " + stats.hits + " " + big);
    }
}
