import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.ListIterator;
import java.util.regex.Pattern;

class Main {
    static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));

    public static void main(String[] args) {
        List<String> lines_items = STDIN.lines().toList();
        ListIterator<String> lines = lines_items.listIterator();
        var t = Integer.parseUnsignedInt(lines.next());
        for (int i = 0; i < t; i++) {
            List<Integer> cs = Arrays.stream(lines.next().split(Pattern.quote(" "), -1)).map(c -> Integer.parseUnsignedInt(c)).toList();
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
