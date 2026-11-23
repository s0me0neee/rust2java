import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.ListIterator;

class Main {
    static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));

    public static void main(String[] args) {
        ListIterator<String> lines = STDIN.lines().toList().listIterator();
        var t = Integer.parseInt(lines.next());
        for (int i = 0; i < t; i++) {
            List<Integer> cs = Arrays.stream(lines.next().split(" ", -1)).map(Integer::parseInt).toList();
            var hash = new HashSet<String>();
            for (int j = 0; j < cs.get(0); j++) {
                hash.add(lines.next());
            }
            for (int k = 0; k < cs.get(1); k++) {
                hash.remove(lines.next());
            }
            for (String s : hash) {
                System.out.println(s);
            }
        }
    }
}
