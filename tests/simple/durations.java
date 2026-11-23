import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.ListIterator;

class Main {
    static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));

    public static void main(String[] args) {
        ListIterator<String> lines = STDIN.lines().toList().listIterator();
        var t = Integer.parseInt(lines.next());
        for (int i = 0; i < t; i++) {
            String l = lines.next();
            List<String> lr = new ArrayList<>(Arrays.asList(l.split(",", -1)));
            String n = lr.get(0);
            lr.remove(0);
            int t2 = 0;
            for (String r : lr) {
                ListIterator<String> a = Arrays.asList(r.split(":", -1)).listIterator();
                String h = a.next();
                String m = a.next();
                t2 += Integer.parseInt(m);
                t2 += Integer.parseInt(h) * 60;
            }
            System.out.print(n + "=");
            int h = t2 / 60;
            int m = t2 % 60;
            System.out.print(h + " " + (h == 1 ? "hour" : "hours") + " ");
            if (m == 0) {
                System.out.println();
                continue;
            }
            System.out.println(m + " " + (m == 1 ? "mintue" : "mintues"));
        }
    }
}
